//! The window: the title bar, the view, the keybar and the key list. Drawn
//! with the painter on the rects it lays out itself, so nothing here scrolls
//! (docs/design.md: no scrollbar anywhere).

use egui::{Align2, Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, ViewportCommand, pos2, vec2};
use ito_common::Clock;

use crate::app::{App, GLOBAL_KEYS, Mode, View};

/// The accent of the tabs, the palette and the best key.
pub const ACCENT: Color32 = Color32::from_rgb(0x5B, 0x9F, 0xD1);

const TITLE_H: f32 = 40.0;
const KEYBAR_H: f32 = 28.0;
/// How many of a view's keys the keybar shows before "? all keys".
const KEYBAR_KEYS: usize = 5;

/// What the frame asks of the app.
#[derive(Default)]
pub struct Out {
    pub view: Option<View>,
    pub settings: bool,
    /// A card clicked, and a card dropped on a column.
    pub pick: Option<usize>,
    pub drop: Option<(usize, usize)>,
}

pub fn frame(ui: &mut Ui, app: &App) -> Out {
    let c = ito_theme::colors();
    let rect = ui.max_rect();
    let mut out = Out::default();
    ui.painter().rect_filled(rect, 0.0, c.bg);
    let title = Rect::from_min_size(rect.min, vec2(rect.width(), TITLE_H));
    let keybar = Rect::from_min_max(pos2(rect.left(), rect.bottom() - KEYBAR_H), rect.max);
    let body = Rect::from_min_max(title.left_bottom(), keybar.left_top());
    title_bar(ui, app, title, &mut out);
    match app.mode {
        Mode::Help => help(ui, app, body),
        _ if app.view == View::Inbox => crate::inbox_ui::draw(ui, app, body),
        _ if app.view == View::List => crate::list_ui::draw(ui, app, body),
        _ if app.view == View::Actions => crate::actions_ui::draw(ui, app, body),
        _ if app.view == View::Board => crate::board_ui::draw(ui, app, body, &mut out),
        _ => empty_view(ui, app, body),
    }
    key_bar(ui, app, keybar);
    ui.painter().line_segment([title.left_bottom(), title.right_bottom()], Stroke::new(1.0, c.border));
    ui.painter().line_segment([keybar.left_top(), keybar.right_top()], Stroke::new(1.0, c.border));
    if let Some(left) = app.toast_left() {
        toast(ui, app, body);
        ui.ctx().request_repaint_after(left);
    }
    out
}

fn title_bar(ui: &mut Ui, app: &App, r: Rect, out: &mut Out) {
    let ctx = ui.ctx().clone();
    let c = ito_theme::colors();
    // The whole bar drags the window; what is drawn over it is registered
    // later, so it takes the pointer first.
    let drag = ui.interact(r, ui.id().with("title-drag"), Sense::click_and_drag());
    if drag.drag_started() {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if drag.double_clicked() {
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
    let p = ui.painter().clone();
    let mut x = r.left() + 12.0;
    let mid = r.center().y;
    // The logo, then the name (no kanji: the mark and the word only).
    let logo = Rect::from_center_size(pos2(x + 11.0, mid), vec2(22.0, 22.0));
    if let Some(tex) = logo_texture(&ctx) {
        p.image(tex.id(), logo, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    x = logo.right() + 8.0;
    let name = p.text(pos2(x, mid), Align2::LEFT_CENTER, "minato", FontId::proportional(14.0), c.fg);
    x = name.right() + 18.0;
    // The view tabs, each with its digit.
    for v in View::ALL {
        let label = app.tr(v.label());
        let font = FontId::proportional(13.0);
        let digit = ui.fonts_mut(|f| f.layout_no_wrap(v.digit().to_string(), FontId::monospace(11.0), c.dim));
        let text = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font, c.fg));
        let w = 10.0 + digit.size().x + 6.0 + text.size().x + 10.0;
        let tab = Rect::from_min_size(pos2(x, mid - 14.0), vec2(w, 28.0));
        let resp = ui.interact(tab, ui.id().with(("tab", v.digit())), Sense::click());
        let on = app.is_on(v);
        if on {
            p.rect_filled(tab, CornerRadius::same(6), c.panel);
            p.line_segment([tab.left_bottom() + vec2(8.0, 0.0), tab.right_bottom() - vec2(8.0, 0.0)], Stroke::new(2.0, ACCENT));
        } else if resp.hovered() {
            p.rect_filled(tab, CornerRadius::same(6), c.panel.gamma_multiply(0.6));
        }
        let (dc, tc) = if on { (ACCENT, c.fg) } else { (c.dim, c.dim) };
        p.galley(pos2(tab.left() + 10.0, mid - digit.size().y / 2.0), digit.clone(), dc);
        p.galley(pos2(tab.left() + 10.0 + digit.size().x + 6.0, mid - text.size().y / 2.0), text, tc);
        if resp.clicked() {
            out.view = Some(v);
        }
        if resp.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        x = tab.right() + 2.0;
    }
    let left_end = x;
    // The window buttons on the right.
    let mut right = r.right();
    for (k, kind) in [Caption::Close, Caption::Maximize, Caption::Minimize].into_iter().enumerate() {
        let b = Rect::from_min_max(pos2(right - 46.0, r.top()), pos2(right, r.bottom() - 1.0));
        let resp = ui.interact(b, ui.id().with(("caption", k)), Sense::click());
        if resp.hovered() {
            let fill = if kind == Caption::Close { Color32::from_rgb(0xC4, 0x2B, 0x1C) } else { c.panel };
            p.rect_filled(b, 0.0, fill);
        }
        let ink = if resp.hovered() && kind == Caption::Close { Color32::WHITE } else { c.fg };
        caption_glyph(&p, b.center(), kind, ink, ctx.input(|i| i.viewport().maximized.unwrap_or(false)));
        if resp.clicked() {
            match kind {
                Caption::Close => ctx.send_viewport_cmd(ViewportCommand::Close),
                Caption::Minimize => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
                Caption::Maximize => {
                    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                }
            }
        }
        right = b.left();
    }
    right -= 8.0;
    // The account: a round mark that will hold the avatar.
    let acct = Rect::from_center_size(pos2(right - 12.0, mid), vec2(24.0, 24.0));
    let resp = ui.interact(acct, ui.id().with("account"), Sense::click());
    p.circle_filled(acct.center(), 11.0, c.border);
    let initial = app.viewer.as_deref().and_then(|l| l.chars().next()).map_or('?'.to_string(), |ch| ch.to_uppercase().to_string());
    p.text(acct.center(), Align2::CENTER_CENTER, initial, FontId::proportional(12.0), if app.viewer.is_some() { c.fg } else { c.dim });
    resp.on_hover_text(app.viewer.as_deref().unwrap_or(app.tr("account")));
    right = acct.left() - 10.0;
    // The bell with the unread count.
    let bell = Rect::from_center_size(pos2(right - 12.0, mid), vec2(26.0, 26.0));
    let resp = ui.interact(bell, ui.id().with("bell"), Sense::click());
    if resp.hovered() {
        p.rect_filled(bell, CornerRadius::same(6), c.panel);
    }
    draw_bell(&p, bell.center(), c.fg);
    if app.unread > 0 {
        let badge = Rect::from_center_size(bell.right_top() + vec2(-3.0, 5.0), vec2(16.0, 14.0));
        p.rect_filled(badge, CornerRadius::same(7), c.err);
        p.text(badge.center(), Align2::CENTER_CENTER, app.unread.min(99).to_string(), FontId::proportional(10.0), Color32::WHITE);
    }
    resp.on_hover_text(app.tr("bell"));
    right = bell.left() - 10.0;
    // The comma opens the settings.
    let gear = Rect::from_center_size(pos2(right - 12.0, mid), vec2(26.0, 26.0));
    let resp = ui.interact(gear, ui.id().with("gear"), Sense::click());
    if resp.hovered() || app.mode == Mode::Settings {
        p.rect_filled(gear, CornerRadius::same(6), c.panel);
    }
    p.text(gear.center(), Align2::CENTER_CENTER, ",", FontId::monospace(15.0), if app.mode == Mode::Settings { ACCENT } else { c.fg });
    if resp.clicked() {
        out.settings = true;
    }
    resp.on_hover_text(app.tr("view.settings"));
    let right_start = gear.left() - 12.0;
    // The palette field in the middle of what is left.
    let free = Rect::from_min_max(pos2(left_end + 12.0, r.top()), pos2(right_start, r.bottom()));
    let w = (free.width()).clamp(0.0, 420.0);
    if w > 120.0 {
        let field = Rect::from_center_size(pos2(r.center().x.clamp(free.left() + w / 2.0, free.right() - w / 2.0), mid), vec2(w, 26.0));
        let resp = ui.interact(field, ui.id().with("palette"), Sense::click());
        p.rect_filled(field, CornerRadius::same(7), if resp.hovered() { c.panel } else { c.side });
        p.rect_stroke(field, CornerRadius::same(7), Stroke::new(1.0, c.border), StrokeKind::Inside);
        p.text(field.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, app.tr("palette.hint"), FontId::proportional(12.5), c.dim);
        p.text(field.right_center() - vec2(10.0, 0.0), Align2::RIGHT_CENTER, "Ctrl+Shift+P", FontId::monospace(11.0), c.dim);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Caption {
    Minimize,
    Maximize,
    Close,
}

fn caption_glyph(p: &egui::Painter, at: Pos2, kind: Caption, ink: Color32, maximized: bool) {
    let s = Stroke::new(1.0, ink);
    match kind {
        Caption::Minimize => {
            p.line_segment([at - vec2(5.0, 0.0), at + vec2(5.0, 0.0)], s);
        }
        Caption::Maximize if maximized => {
            p.rect_stroke(Rect::from_center_size(at + vec2(-1.0, 1.0), vec2(8.0, 8.0)), 0.0, s, StrokeKind::Middle);
            p.line_segment([at + vec2(-2.0, -4.5), at + vec2(4.5, -4.5)], s);
            p.line_segment([at + vec2(4.5, -4.5), at + vec2(4.5, 2.0)], s);
        }
        Caption::Maximize => {
            p.rect_stroke(Rect::from_center_size(at, vec2(9.0, 9.0)), 0.0, s, StrokeKind::Middle);
        }
        Caption::Close => {
            p.line_segment([at - vec2(4.5, 4.5), at + vec2(4.5, 4.5)], s);
            p.line_segment([at + vec2(-4.5, 4.5), at + vec2(4.5, -4.5)], s);
        }
    }
}

fn draw_bell(p: &egui::Painter, at: Pos2, ink: Color32) {
    let s = Stroke::new(1.4, ink);
    let pts = [
        at + vec2(-5.5, 4.0),
        at + vec2(-4.0, 2.5),
        at + vec2(-4.0, -1.5),
        at + vec2(-2.5, -4.5),
        at + vec2(0.0, -5.5),
        at + vec2(2.5, -4.5),
        at + vec2(4.0, -1.5),
        at + vec2(4.0, 2.5),
        at + vec2(5.5, 4.0),
    ];
    p.add(egui::Shape::line(pts.to_vec(), s));
    p.line_segment([pts[0], pts[8]], s);
    p.line_segment([at + vec2(-1.5, 6.5), at + vec2(1.5, 6.5)], s);
}

/// The logo as a texture, made once from the icon's own drawing.
fn logo_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::new("minato-logo");
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(t);
    }
    const PX: u32 = 64;
    let image = egui::ColorImage::from_rgba_unmultiplied([PX as usize; 2], &crate::icon::pixels(PX));
    let tex = ctx.load_texture("minato-logo", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    Some(tex)
}

/// A view that has nothing to show yet.
fn empty_view(ui: &mut Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let p = ui.painter();
    let mid = r.center();
    p.text(mid - vec2(0.0, 14.0), Align2::CENTER_CENTER, app.tr(app.view.label()), FontId::proportional(20.0), c.fg);
    p.text(mid + vec2(0.0, 12.0), Align2::CENTER_CENTER, app.tr(app.view.empty()), FontId::proportional(13.0), c.dim);
    p.text(mid + vec2(0.0, 34.0), Align2::CENTER_CENTER, app.tr("empty.soon"), FontId::proportional(12.0), c.dim.gamma_multiply(0.7));
}

/// The keybar: the keys of the view at the left, the best one lit; the
/// state of the sync and the clock at the right.
fn key_bar(ui: &mut Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let p = ui.painter().clone();
    let mid = r.center().y;
    let mut x = r.left() + 12.0;
    let keys: Vec<(&str, &str)> = match app.mode {
        Mode::Settings => vec![("Esc", "key.back"), ("Ctrl+Tab", "settings.theme")],
        Mode::Help => vec![("?", "key.back"), ("Esc", "key.back")],
        Mode::Main => app.view.keys().iter().take(KEYBAR_KEYS).copied().chain([("?", "key.help")]).collect(),
    };
    for (k, (key, word)) in keys.iter().enumerate() {
        let best = k == 0;
        let kc = if best { ACCENT } else { c.fg };
        let galley = ui.fonts_mut(|f| f.layout_no_wrap((*key).to_owned(), FontId::monospace(11.0), kc));
        let chip = Rect::from_min_size(pos2(x, mid - 9.0), vec2(galley.size().x + 12.0, 18.0));
        p.rect_filled(chip, CornerRadius::same(4), if best { ACCENT.gamma_multiply(0.18) } else { c.panel });
        p.rect_stroke(chip, CornerRadius::same(4), Stroke::new(1.0, if best { ACCENT.gamma_multiply(0.6) } else { c.border }), StrokeKind::Inside);
        p.galley(chip.min + vec2(6.0, 9.0 - galley.size().y / 2.0), galley, kc);
        let label = word_for(app, word);
        let t = p.text(pos2(chip.right() + 6.0, mid), Align2::LEFT_CENTER, label, FontId::proportional(12.0), if best { c.fg } else { c.dim });
        x = t.right() + 14.0;
    }
    // The right side, from the right: the clock, the API budget, the sync.
    let mut right = r.right() - 12.0;
    let clock = app.prefs.common.clock_or(None);
    let now = chrono::Local::now().naive_local();
    if clock.show {
        let t = p.text(pos2(right, mid), Align2::RIGHT_CENTER, clock.text(&now, &app.lang), FontId::monospace(12.0), c.fg);
        right = t.left() - 16.0;
        ui.ctx().request_repaint_after(Clock::next_minute(&now));
    }
    let api = minato_core::status::api(app.api);
    let t = p.text(pos2(right, mid), Align2::RIGHT_CENTER, api, FontId::monospace(11.5), c.dim);
    right = t.left() - 16.0;
    let ago = minato_core::status::ago(app.since_sync(), app.ja());
    p.text(pos2(right, mid), Align2::RIGHT_CENTER, format!("⟳ {ago}"), FontId::monospace(11.5), c.dim);
    if app.synced.is_some() {
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
    }
}

/// A key's words: a view-key word, or (for the settings' own entries) the
/// settings table's.
fn word_for(app: &App, key: &'static str) -> &'static str {
    app.tr(key)
}

/// The list of every key: the global ones, then the view's.
fn help(ui: &mut Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let p = ui.painter().clone();
    let w = r.width().min(640.0);
    let card = Rect::from_center_size(r.center(), vec2(w, (r.height() - 24.0).clamp(80.0, 420.0)));
    p.rect_filled(card, CornerRadius::same(10), c.raised());
    p.rect_stroke(card, CornerRadius::same(10), Stroke::new(1.0, c.border_strong()), StrokeKind::Inside);
    let mut y = card.top() + 18.0;
    p.text(pos2(card.left() + 20.0, y), Align2::LEFT_TOP, app.tr("help.title"), FontId::proportional(16.0), c.fg);
    y += 32.0;
    let section = |title: &str, rows: &[(&str, &'static str)], y: &mut f32| {
        p.text(pos2(card.left() + 20.0, *y), Align2::LEFT_TOP, title, FontId::proportional(11.5), ACCENT);
        *y += 20.0;
        for (key, word) in rows {
            if *y + 18.0 > card.bottom() - 28.0 {
                return;
            }
            p.text(pos2(card.left() + 20.0, *y), Align2::LEFT_TOP, *key, FontId::monospace(12.0), c.fg);
            p.text(pos2(card.left() + 150.0, *y), Align2::LEFT_TOP, app.tr(word), FontId::proportional(12.5), c.dim);
            *y += 20.0;
        }
        *y += 8.0;
    };
    section(app.tr("help.global"), &GLOBAL_KEYS, &mut y);
    section(app.tr(app.view.label()), app.view.keys(), &mut y);
    p.text(pos2(card.left() + 20.0, card.bottom() - 22.0), Align2::LEFT_CENTER, app.tr("help.close"), FontId::proportional(11.5), c.dim);
}

fn toast(ui: &mut Ui, app: &App, r: Rect) {
    let Some((text, _)) = &app.toast else { return };
    let c = ito_theme::colors();
    let p = ui.painter().clone();
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(text.clone(), FontId::proportional(12.5), c.fg));
    let box_ = Rect::from_center_size(pos2(r.center().x, r.bottom() - 24.0), galley.size() + vec2(28.0, 14.0));
    p.rect_filled(box_, CornerRadius::same(8), c.raised());
    p.rect_stroke(box_, CornerRadius::same(8), Stroke::new(1.0, c.border_strong()), StrokeKind::Inside);
    p.galley(box_.min + vec2(14.0, 7.0), galley, c.fg);
}

/// A window without the system's frame is resized by its edges: a strip
/// along each, and the corners, set the pointer and start the system's
/// resizing on the press. Called last in a frame, so its pointer is the one
/// shown. Not while maximized.
pub fn resize_edges(ctx: &egui::Context) {
    use egui::{CursorIcon as C, ResizeDirection as D};
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let (pos, pressed) = ctx.input(|i| (i.pointer.latest_pos(), i.pointer.primary_pressed()));
    let Some(p) = pos else { return };
    let Some(dir) = edge_at(ctx.content_rect(), p) else { return };
    ctx.set_cursor_icon(match dir {
        D::North | D::South => C::ResizeVertical,
        D::East | D::West => C::ResizeHorizontal,
        D::NorthWest | D::SouthEast => C::ResizeNwSe,
        D::NorthEast | D::SouthWest => C::ResizeNeSw,
    });
    if pressed {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
    }
}

/// Which edge or corner of `r` the point `p` is on, if any. The top edge is
/// thinner: the band under it is the title bar to drag by.
fn edge_at(r: Rect, p: Pos2) -> Option<egui::ResizeDirection> {
    use egui::ResizeDirection as D;
    const EDGE: f32 = 6.0;
    const CORNER: f32 = 12.0;
    if !r.contains(p) {
        return None;
    }
    let (left, right) = (p.x - r.left() < EDGE, r.right() - p.x < EDGE);
    let (top, bottom) = (p.y - r.top() < EDGE / 2.0, r.bottom() - p.y < EDGE);
    let (near_left, near_right) = (p.x - r.left() < CORNER, r.right() - p.x < CORNER);
    let (near_top, near_bottom) = (p.y - r.top() < CORNER, r.bottom() - p.y < CORNER);
    let dir = match () {
        _ if (top && near_left) || (left && near_top) => D::NorthWest,
        _ if (top && near_right) || (right && near_top) => D::NorthEast,
        _ if (bottom && near_left) || (left && near_bottom) => D::SouthWest,
        _ if (bottom && near_right) || (right && near_bottom) => D::SouthEast,
        _ if top => D::North,
        _ if bottom => D::South,
        _ if left => D::West,
        _ if right => D::East,
        _ => return None,
    };
    Some(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::ResizeDirection as D;

    #[test]
    fn edges_and_corners() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        assert_eq!(edge_at(r, pos2(400.0, 300.0)), None);
        assert_eq!(edge_at(r, pos2(400.0, 599.0)), Some(D::South));
        assert_eq!(edge_at(r, pos2(1.0, 300.0)), Some(D::West));
        assert_eq!(edge_at(r, pos2(799.0, 599.0)), Some(D::SouthEast));
        assert_eq!(edge_at(r, pos2(1.0, 1.0)), Some(D::NorthWest));
        // Under the thin top edge is the title bar.
        assert_eq!(edge_at(r, pos2(400.0, 5.0)), None);
        assert_eq!(edge_at(r, pos2(900.0, 5.0)), None);
    }
}
