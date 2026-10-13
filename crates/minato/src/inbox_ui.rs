//! The inbox view (docs/design.md, 「1 受信箱」), as the mock draws it: the
//! queue at the left (the head, the label chips, the three sections of
//! cards, the status and repository chips at the foot) and the detail at the
//! right. Painted on rects; the cards fold (`minato_core::fold`) instead of
//! scrolling. A click does what the keys that get there do.

use egui::{Align2, Color32, CornerRadius, FontId, Painter, Rect, Stroke, StrokeKind, Ui, pos2, vec2};
use minato_core::fold::{Show, fold};
use minato_core::inbox::{Card, Group, InboxState, Status};
use minato_core::list::{Label, age, epoch};

use crate::app::{App, Click, Pane, Wheel, now_epoch};
use crate::style::{self, ACCENT};
use crate::ui::{self, Out};

/// A card and the 6 px under it.
const CARD_FULL: f32 = 102.0;
const CARD_GAP: f32 = 6.0;
const MORE_H: f32 = 22.0;
/// A section's header, and the 「なし」 of an empty one.
const SEC_H: f32 = 30.0;
const EMPTY_H: f32 = 24.0;
/// The foot: two rows of chips.
/// The rows the foot's chips may take before the rest are left out.
const FOOT_ROWS: usize = 4;
const CHIP_H: f32 = 22.0;
/// The border of a chip that is on.
const CHIP_ON: Color32 = Color32::from_rgb(0x57, 0x55, 0x7A);

pub fn draw(ui: &mut Ui, app: &App, r: Rect, out: &mut Out) {
    for (pane, rect) in ui::panes(&app.inbox_split, r) {
        let at = if pane == Pane::Left { Wheel::Inbox } else { Wheel::Detail };
        ui::wheel(ui.ctx(), ui.layer_id(), rect, at, out);
        match pane {
            Pane::Left => queue(ui, app, rect, out),
            _ => crate::detail_ui::draw(ui, app, rect, app.inbox.scroll, out),
        }
    }
    ui::dividers(ui, &app.inbox_split, r, out);
}

fn queue(ui: &mut Ui, app: &App, r: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(r, 0.0, c.side);
    let all = app.inbox_all();
    let shown = app.inbox.shown(&all, &app.items);
    // The head: 受信箱 N of M, and the order on the right.
    let hy = r.top() + 12.0 + 9.0;
    let t = style::bold(&p, pos2(r.left() + 14.0, hy), Align2::LEFT_CENTER, app.tr("view.inbox"), FontId::proportional(11.5), c.dim);
    p.text(pos2(t.right() + 8.0, hy), Align2::LEFT_CENTER, format!("{} of {}", shown.len(), all.len()), FontId::proportional(12.0), c.fg);
    let sort = app.tr("inbox.sort");
    let sw = p.layout_no_wrap(sort.to_owned(), FontId::proportional(11.5), c.fg).size().x + 18.0;
    style::chip(&p, r.right() - 14.0 - sw, hy, sort, FontId::proportional(11.5), 9.0, 20.0, 6, c.border, Color32::TRANSPARENT, c.fg);
    let y = tags(ui, &p, app, &all, r, r.top() + 12.0 + 20.0, out);
    let foot = chips(ui, &p, app, &all, r, out);
    let body = Rect::from_min_max(pos2(r.left() + 8.0, y), pos2(r.right() - 8.0, foot - 4.0));
    if all.is_empty() {
        let words = app.tr(if app.viewer.is_none() { "list.loading" } else { "inbox.empty" });
        p.text(pos2(body.left() + 6.0, body.top() + 20.0), Align2::LEFT_CENTER, words, FontId::proportional(13.0), c.dim);
        return;
    }
    sections(ui, &p.with_clip_rect(body), app, &shown, body, out);
}

/// The labels of the cards as chips that pick one; where the row ends.
fn tags(ui: &Ui, p: &Painter, app: &App, all: &[Card], r: Rect, top: f32, out: &mut Out) -> f32 {
    let c = ito_theme::colors();
    let mut seen: Vec<&Label> = Vec::new();
    for card in all {
        for l in &app.items[card.at].labels {
            if !seen.iter().any(|s| s.name == l.name) {
                seen.push(l);
            }
        }
    }
    seen.sort_by(|a, b| a.name.cmp(&b.name));
    if seen.is_empty() {
        return top + 6.0;
    }
    let (left, right) = (r.left() + 14.0, r.right() - 14.0);
    let (mut x, mut y) = (left, top + 6.0 + 9.0);
    for l in seen {
        let w = p.layout_no_wrap(l.name.clone(), FontId::proportional(11.0), Color32::WHITE).size().x + 14.0;
        if x + w > right && x > left {
            x = left;
            y += 18.0 + 5.0;
        }
        let rect = Rect::from_min_size(pos2(x, y - 9.0), vec2(w, 18.0));
        let on = app.inbox.tag.as_deref() == Some(l.name.as_str());
        let mut q = p.clone();
        if app.inbox.tag.is_some() && !on {
            q.set_opacity(0.35);
        }
        style::label(&q, x, y, l);
        if on {
            p.rect_stroke(rect.expand(1.5), CornerRadius::same(10), Stroke::new(1.5, c.fg), StrokeKind::Inside);
        }
        if ui::hit(ui, rect, ("inbox-tag", &l.name)).clicked() {
            out.click = Some(Click::InboxTag(Some(l.name.clone())));
        }
        x += w + 5.0;
    }
    y + 9.0 + 6.0
}

/// The sections あなたの番, 待っている and 今日済んだ that the status lets
/// through, with their cards folded to fit.
fn sections(ui: &Ui, p: &Painter, app: &App, shown: &[Card], body: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let groups: Vec<Group> = [Group::Mine, Group::Wait, Group::Done].into_iter().filter(|g| app.inbox.status.takes(*g)).collect();
    let empties = groups.iter().filter(|g| !shown.iter().any(|card| card.group == **g)).count();
    let room = body.height() - groups.len() as f32 * SEC_H - empties as f32 * EMPTY_H;
    let sel = app.inbox.sel.min(shown.len().saturating_sub(1));
    let f = fold(&vec![CARD_FULL; shown.len()], MORE_H, room, sel);
    let now = now_epoch();
    let mut y = body.top();
    if f.above > 0 {
        more(p, app, body.left(), y, f.above, "list.above");
        y += MORE_H;
    }
    for g in groups {
        let (name, hint) = match g {
            Group::Mine => ("inbox.mine", "inbox.mine_hint"),
            Group::Wait => ("inbox.wait", "inbox.wait_hint"),
            Group::Done => ("inbox.done", ""),
        };
        let n = shown.iter().filter(|card| card.group == g).count();
        let sy = y + 10.0 + 7.0;
        let t = style::bold(p, pos2(body.left() + 6.0, sy), Align2::LEFT_CENTER, app.tr(name), FontId::proportional(12.0), c.fg);
        p.text(pos2(t.right() + 6.0, sy), Align2::LEFT_CENTER, n.to_string(), FontId::monospace(12.0), c.dim);
        if !hint.is_empty() {
            p.text(pos2(body.right() - 6.0, sy), Align2::RIGHT_CENTER, app.tr(hint), FontId::proportional(11.5), c.dim);
        }
        y += SEC_H;
        if n == 0 {
            p.text(pos2(body.left() + 6.0, y + EMPTY_H / 2.0 - 4.0), Align2::LEFT_CENTER, app.tr("inbox.none"), FontId::proportional(12.5), c.dim);
            y += EMPTY_H;
            continue;
        }
        for (k, card) in shown.iter().enumerate().filter(|(_, card)| card.group == g) {
            if f.rows[k] == Show::Hidden {
                continue;
            }
            let rect = Rect::from_min_size(pos2(body.left(), y), vec2(body.width(), CARD_FULL - CARD_GAP));
            let resp = ui::hit(ui, rect, ("inbox-card", k));
            if resp.clicked() {
                out.click = Some(Click::InboxCard(k));
            }
            draw_card(p, app, card, rect, k == sel, resp.hovered(), now);
            ui::peek(&resp, &app.items[card.at].peek);
            y = rect.bottom() + CARD_GAP;
        }
    }
    if f.below > 0 {
        more(p, app, body.left(), y, f.below, "list.below");
    }
}

/// The tone of a card's left edge and its reason: the rule's, else
/// waiting's or done's.
pub fn card_tone(card: &Card) -> Color32 {
    match (card.group, card.rule) {
        (Group::Mine, Some(rule)) => style::tone(rule.tone()),
        (Group::Wait, _) => style::RUN,
        (Group::Done, _) => style::OK,
        _ => style::IDLE,
    }
}

/// What a card says it is there for.
pub fn card_why(app: &App, card: &Card) -> &'static str {
    match (card.rule, card.group) {
        (Some(rule), _) => app.tr(rule.why()),
        (None, Group::Wait) => app.tr("why.waiting"),
        (None, _) => app.tr("inbox.done"),
    }
}

/// One card: kind, title and age; where and who; why in its tone; the
/// repository and the labels.
fn draw_card(p: &Painter, app: &App, card: &Card, r: Rect, on: bool, hover: bool, now: i64) {
    let c = ito_theme::colors();
    let item = &app.items[card.at];
    let tone = card_tone(card);
    let mut p = p.clone();
    if card.group == Group::Done {
        p.set_opacity(0.55);
    }
    let fill = if on { style::sel() } else if hover { c.hover() } else { style::card() };
    p.rect_filled(r, CornerRadius::same(8), fill);
    p.rect_stroke(r, CornerRadius::same(8), Stroke::new(if on { 2.0 } else { 1.0 }, if on { ACCENT } else { c.border }), StrokeKind::Inside);
    p.rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), CornerRadius { nw: 8, sw: 8, ne: 0, se: 0 }, tone);
    let x = r.left() + 13.0;
    let right = r.right() - 10.0;
    let y1 = r.top() + 7.0 + 10.0;
    let kx = style::kind_chip(&p, x, y1, item.kind) + 7.0;
    let when = epoch(&item.updated_at).map(|t| age(&item.updated_at, now.max(t))).unwrap_or_default();
    let t = p.text(pos2(right, y1), Align2::RIGHT_CENTER, when, FontId::monospace(11.5), c.dim);
    // The marked one first, so its ground is under both.
    let title = style::truncated_lit(&p, item.title.clone(), FontId::proportional(13.0), c.fg, t.left() - 7.0 - kx, app.lit());
    let ty = y1 - title.size().y / 2.0;
    p.galley(pos2(kx, ty), title, c.fg);
    let title = style::truncated(&p, item.title.clone(), FontId::proportional(13.0), c.fg, t.left() - 7.0 - kx);
    p.galley(pos2(kx + 0.45, ty), title, c.fg);
    let y2 = y1 + 20.0;
    let place = if minato_core::mainci::is_stub(item) { item.repo.clone() } else { format!("{} #{} · {}", item.repo, item.number, item.author) };
    let g = style::truncated(&p, place, FontId::monospace(12.0), c.dim, right - x);
    p.galley(pos2(x, y2 - g.size().y / 2.0), g, c.dim);
    let y3 = y2 + 19.0;
    let d = p.text(pos2(x, y3), Align2::LEFT_CENTER, "●", FontId::proportional(9.0), tone);
    let g = style::truncated(&p, card_why(app, card).to_owned(), FontId::proportional(12.5), tone, right - d.right() - 6.0);
    p.galley(pos2(d.right() + 6.0, y3 - g.size().y / 2.0), g, tone);
    let y4 = y3 + 23.0;
    let lx = style::tag(&p, x, y4, &item.repo, item.repo_name()) + 4.0;
    style::labels(&p, lx, y4, &item.labels, right);
}

fn more(p: &Painter, app: &App, x: f32, y: f32, n: usize, side: &'static str) {
    style::more(p, app.ja(), app.tr(side), x + 6.0, y + MORE_H / 2.0, n);
}

/// One chip of the foot before it is placed.
struct FootChip<'a> {
    dot: Option<Color32>,
    word: &'a str,
    n: usize,
    on: bool,
    click: Click,
}

/// The foot at the bottom of `r`: the status chips, then the repository
/// chips, each with how many cards it would show; a group wraps to more
/// rows when the column is narrow (up to `FOOT_ROWS`). Where the foot starts.
fn chips(ui: &Ui, p: &Painter, app: &App, all: &[Card], r: Rect, out: &mut Out) -> f32 {
    let c = ito_theme::colors();
    let count = |s: InboxState| s.shown(all, &app.items).len();
    let base = || InboxState { undo: Vec::new(), ..app.inbox.clone() };
    let sts = [(Status::All, "inbox.all", None), (Status::Mine, "inbox.mine", Some(ACCENT)), (Status::Wait, "inbox.wait", Some(style::RUN)), (Status::Done, "inbox.done_chip", Some(style::OK))];
    let status: Vec<FootChip> = sts
        .into_iter()
        .map(|(st, word, dot)| FootChip { dot, word: app.tr(word), n: count(InboxState { status: st, ..base() }), on: app.inbox.status == st, click: Click::InboxStatus(st) })
        .collect();
    let (repos, _) = InboxState::repos_and_tags(all, &app.items);
    let repos: Vec<FootChip> = std::iter::once(None)
        .chain(repos.iter().map(|r| Some(r.as_str())))
        .map(|repo| FootChip {
            dot: None,
            word: repo.map_or(app.tr("inbox.all_repos"), |r| r.rsplit('/').next().unwrap_or(r)),
            n: count(InboxState { repo: repo.map(str::to_owned), ..base() }),
            on: app.inbox.repo.as_deref() == repo,
            click: Click::InboxRepo(repo.map(str::to_owned)),
        })
        .collect();
    // Place them first, so the foot is as tall as its rows.
    let (left, right) = (r.left() + 12.0, r.right() - 12.0);
    let mut at = Vec::new();
    let mut row = 0;
    for group in [&status, &repos] {
        let mut x = left;
        for f in group.iter() {
            let w = chip_w(p, f.dot, f.word, f.n);
            if x + w > right && x > left {
                x = left;
                row += 1;
            }
            at.push((x, row));
            x += w + 5.0;
        }
        row += 1;
    }
    let top = r.bottom() - 8.0 - row.min(FOOT_ROWS) as f32 * (CHIP_H + 8.0);
    p.line_segment([pos2(r.left(), top), pos2(r.right(), top)], Stroke::new(1.0, c.border));
    for (k, (f, (x, row))) in status.iter().chain(&repos).zip(at).enumerate() {
        if row >= FOOT_ROWS {
            break;
        }
        let y = top + 8.0 + CHIP_H / 2.0 + row as f32 * (CHIP_H + 8.0);
        if chip(ui, p, x, y, f.dot, f.word, f.n, f.on, ("inbox-foot", k)).clicked() {
            out.click = Some(f.click.clone());
        }
    }
    top
}

/// How wide a chip of the foot is.
fn chip_w(p: &Painter, dot: Option<Color32>, word: &str, n: usize) -> f32 {
    let w = |s: String, font: FontId| p.layout_no_wrap(s, font, Color32::WHITE).size().x;
    9.0 + dot.map_or(0.0, |_| w("●".into(), FontId::proportional(8.0)) + 5.0) + w(word.to_owned(), FontId::proportional(12.0)) + 5.0 + w(n.to_string(), FontId::monospace(11.0)) + 9.0
}

/// A chip of the foot: its dot, its words and its count; its response.
#[allow(clippy::too_many_arguments)]
fn chip(ui: &Ui, p: &Painter, x: f32, y: f32, dot: Option<Color32>, word: &str, n: usize, on: bool, id: impl std::hash::Hash + std::fmt::Debug) -> egui::Response {
    let c = ito_theme::colors();
    let color = if on { c.fg } else { c.dim };
    let dot_g = dot.map(|d| p.layout_no_wrap("●".into(), FontId::proportional(8.0), d));
    let word_g = p.layout_no_wrap(word.to_owned(), FontId::proportional(12.0), color);
    let n_g = p.layout_no_wrap(n.to_string(), FontId::monospace(11.0), color);
    let r = Rect::from_min_size(pos2(x, y - CHIP_H / 2.0), vec2(chip_w(p, dot, word, n), CHIP_H));
    let resp = ui::hit(ui, r, id);
    if on {
        p.rect_filled(r, CornerRadius::same(6), style::head());
    }
    p.rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, if resp.hovered() { ACCENT } else if on { CHIP_ON } else { c.border }), StrokeKind::Inside);
    let mut gx = r.left() + 9.0;
    if let Some(g) = dot_g {
        let gw = g.size().x;
        p.galley(pos2(gx, y - g.size().y / 2.0), g, color);
        gx += gw + 5.0;
    }
    let ww = word_g.size().x;
    p.galley(pos2(gx, y - word_g.size().y / 2.0), word_g, color);
    gx += ww + 5.0;
    p.galley(pos2(gx, y - n_g.size().y / 2.0), n_g, color);
    resp
}
