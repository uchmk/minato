//! The window: the title bar, the view, the keybar and the key list. Drawn
//! with the painter on the rects it lays out itself, so nothing here scrolls
//! (docs/design.md: no scrollbar anywhere).

use egui::{pos2, vec2, Align2, Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, ViewportCommand};
use ito_common::Clock;

use ito_layout::Node;
use minato_core::inbox::RULES;

use crate::app::{App, Click, Mode, NoteClick, Pane, View, Wheel, GLOBAL_KEYS};
use crate::keymap::Ctx;

/// The accent of the tabs, the palette and the best key.
pub const ACCENT: Color32 = Color32::from_rgb(0x5B, 0x9F, 0xD1);

const TITLE_H: f32 = 40.0;
const KEYBAR_H: f32 = 28.0;

/// What the frame asks of the app.
#[derive(Default)]
pub struct Out {
    pub view: Option<View>,
    /// The bell was clicked.
    pub bell: bool,
    /// The palette field was clicked.
    pub palette: bool,
    /// A card clicked, and a card dropped on a column.
    pub pick: Option<usize>,
    pub drop: Option<(usize, usize)>,
    /// A card under the mouse whose body has not come (its node id).
    pub peek: Option<String>,
    /// A row, a chip or a column clicked in the view.
    pub click: Option<Click>,
    /// The view's columns as a divider was dragged to.
    pub split: Option<Node<Pane>>,
    /// What the mouse did in the notifications.
    pub note: Option<NoteClick>,
    /// Where the bell is, for the notifications to hang from.
    pub bell_at: Option<Rect>,
    /// The field the detail's strip left for the writing box, while the box
    /// is open on the item it shows.
    pub compose_at: Option<Rect>,
    /// The wheel turned over a list or a detail, by so many notches (up is
    /// plus).
    pub wheel: Option<(Wheel, f32)>,
    /// How far the detail scrolls, in lines, before its end shows.
    pub detail_max: Option<f32>,
    /// The search box's field in the keybar, while it has the keys.
    pub find_at: Option<Rect>,
    /// Where the built-in browser goes (the right of the view), while it is
    /// open and the help is not shown.
    pub browser_at: Option<Rect>,
    /// The browser's width as its edge was dragged to.
    pub browser_w: Option<f32>,
}

/// How far apart the columns are: the 1 px line between them, which the
/// mouse takes from a strip around it.
const GAP: f32 = 1.0;
const GRAB: f32 = 8.0;

/// The columns of a view's split in `r`.
pub fn panes(split: &Node<Pane>, r: Rect) -> Vec<(Pane, Rect)> {
    split.layout(ito_layout::ui::from_egui(r), GAP).into_iter().map(|(p, r)| (p, ito_layout::ui::to_egui(r))).collect()
}

/// The lines between the columns, and their dividers (ito-layout) to drag;
/// called after the columns' rows, so the dividers take the pointer first.
pub fn dividers(ui: &Ui, split: &Node<Pane>, r: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let area = ito_layout::ui::from_egui(r);
    for d in split.dividers(area, GAP) {
        ui.painter().rect_filled(ito_layout::ui::to_egui(d.gap), 0.0, c.border);
    }
    let look = ito_layout::ui::Look { gap: GAP, grab: GRAB, line: ACCENT };
    match ito_layout::ui::dividers(ui, ui.id().with("columns"), split, area, look) {
        Some(ito_layout::ui::Moved::Dragging(n) | ito_layout::ui::Moved::Halved(n)) => out.split = Some(n),
        Some(ito_layout::ui::Moved::Released) | None => {}
    }
}

/// The wheel over `r` on `layer`, into `out.wheel` as notches. Not over a
/// part that scrolls by itself ([`OWN_WHEEL`]).
pub fn wheel(ctx: &egui::Context, layer: egui::LayerId, r: Rect, at: Wheel, out: &mut Out) {
    if !ctx.rect_contains_pointer(layer, r) {
        return;
    }
    let own: Option<(Rect, u64)> = ctx.data(|d| d.get_temp(egui::Id::new(OWN_WHEEL)));
    if own.is_some_and(|(o, pass)| pass + 1 >= ctx.cumulative_pass_nr() && ctx.rect_contains_pointer(layer, o)) {
        return;
    }
    let notches = notches(ctx);
    if notches != 0.0 {
        out.wheel = Some((at, notches));
    }
}

/// Where a part that takes the wheel itself was drawn, and in which pass
/// (kept in the context's data): the view behind does not scroll under it.
pub const OWN_WHEEL: &str = "minato-own-wheel";

/// This frame's wheel as notches (a touchpad's points are 40 a notch). Not
/// with Ctrl: that is the zoom's.
pub fn notches(ctx: &egui::Context) -> f32 {
    ctx.input(|i| {
        i.events
            .iter()
            .map(|e| match e {
                egui::Event::MouseWheel { unit, delta, modifiers, .. } if !modifiers.command => match unit {
                    egui::MouseWheelUnit::Line => delta.y,
                    egui::MouseWheelUnit::Point => delta.y / 40.0,
                    egui::MouseWheelUnit::Page => delta.y * 5.0,
                },
                _ => 0.0,
            })
            .sum()
    })
}

/// Whether a box is over the view this frame (kept in the context's data).
pub const BOXED: &str = "minato-boxed";

/// An item's body while the pointer rests on its card or row, as tsumugi's
/// peek draws a session's last lines: monospace on the ground, 520 wide, a
/// dozen lines at most. Nothing for an item without a body, nor under a box.
pub fn peek(resp: &egui::Response, text: &str) {
    if text.is_empty() || !resp.hovered() || resp.ctx.data(|d| d.get_temp::<bool>(BOXED.into())).unwrap_or(false) {
        return;
    }
    resp.clone().on_hover_ui(|ui| {
        let c = ito_theme::colors();
        let mut job = egui::text::LayoutJob::simple(text.to_owned(), FontId::monospace(11.0), c.fg, 520.0);
        job.wrap.max_rows = 12;
        job.wrap.overflow_character = Some('…');
        let g = ui.fonts_mut(|f| f.layout_job(job));
        let (rect, _) = ui.allocate_exact_size(g.size() + vec2(16.0, 12.0), Sense::hover());
        ui.painter().rect_filled(rect, 6.0, c.bg);
        ui.painter().galley(rect.min + vec2(8.0, 6.0), g, c.fg);
    });
}

/// A rect that takes a click, with the pointing hand over it.
pub fn hit(ui: &Ui, r: Rect, id: impl std::hash::Hash + std::fmt::Debug) -> egui::Response {
    let resp = ui.interact(r, ui.id().with(id), Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp
}

pub fn frame(ui: &mut Ui, app: &App) -> Out {
    let c = ito_theme::colors();
    let rect = ui.max_rect();
    let mut out = Out::default();
    ui.painter().rect_filled(rect, 0.0, c.bg);
    let title = Rect::from_min_size(rect.min, vec2(rect.width(), TITLE_H));
    let keybar = Rect::from_min_max(pos2(rect.left(), rect.bottom() - KEYBAR_H), rect.max);
    let mut body = Rect::from_min_max(title.left_bottom(), pos2(rect.right(), keybar.top()));
    title_bar(ui, app, title, &mut out);
    // The built-in browser takes the right of the view.
    let page = app.browser_w.filter(|_| app.mode != Mode::Help).map(|w| {
        let w = browser_width(w, body.width());
        Rect::from_min_max(pos2(body.right() - w, body.top()), body.max)
    });
    if let Some(page) = page {
        body.max.x = page.left() - GAP;
        out.browser_at = Some(page);
    }
    match app.mode {
        Mode::Help => help(ui, app, body),
        _ if app.view == View::Inbox => crate::inbox_ui::draw(ui, app, body, &mut out),
        _ if app.view == View::List => crate::list_ui::draw(ui, app, body, &mut out),
        _ if app.view == View::Actions => crate::actions_ui::draw(ui, app, body, &mut out),
        _ if app.view == View::Board => crate::board_ui::draw(ui, app, body, &mut out),
        _ if app.view == View::Status => crate::status_ui::draw(ui, app, body, &mut out),
        _ => empty_view(ui, app, body),
    }
    if let Some(page) = page {
        browser_edge(ui, page, &mut out);
    }
    key_bar(ui, app, keybar, &mut out);
    ui.painter().line_segment([title.left_bottom(), title.right_bottom()], Stroke::new(1.0, c.border));
    ui.painter().line_segment([keybar.left_top(), keybar.right_top()], Stroke::new(1.0, c.border));
    // The notifications hang from the bell over the view, as in tsumugi.
    if app.mode == Mode::Notes {
        crate::notes_ui::show(ui, app, &mut out);
    }
    out
}

/// The browser's width in a body `total` wide: at least 320, and the view
/// keeps 300 while there is room for both.
pub fn browser_width(asked: f32, total: f32) -> f32 {
    asked.clamp(320.0, (total - 300.0).max(320.0))
}

/// The line left of the browser and the strip round it to drag; called after
/// the view's rows, so it takes the pointer first.
fn browser_edge(ui: &Ui, page: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let line = Rect::from_min_max(pos2(page.left() - GAP, page.top()), pos2(page.left(), page.bottom()));
    let strip = Rect::from_center_size(line.center(), vec2(GRAB, page.height()));
    let resp = ui.interact(strip, ui.id().with("browser-edge"), Sense::drag());
    let lit = resp.hovered() || resp.dragged();
    ui.painter().rect_filled(line, 0.0, if lit { ACCENT } else { c.border });
    if lit {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
    }
    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
        out.browser_w = Some(page.right() - p.x);
    }
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
    let mid = r.center().y;
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
    // The account: a round mark that will hold the avatar, ringed while
    // another's repositories are shown.
    let acct = Rect::from_center_size(pos2(right - 12.0, mid), vec2(24.0, 24.0));
    let resp = ui.interact(acct, ui.id().with("account"), Sense::click());
    p.circle_filled(acct.center(), 11.0, c.border);
    let viewer = app.viewer.as_deref().filter(|_| !app.signed_out);
    let initial = viewer.and_then(|l| l.chars().next()).map_or('?'.to_string(), |ch| ch.to_uppercase().to_string());
    p.text(acct.center(), Align2::CENTER_CENTER, initial, FontId::proportional(12.0), if viewer.is_some() { c.fg } else { c.dim });
    let mut hover = viewer.map_or_else(|| app.tr("account").into(), str::to_string);
    if !app.settings.owner.is_empty() {
        p.circle_stroke(acct.center(), 11.5, Stroke::new(1.5, ACCENT));
        hover = format!(
            "{hover}
{}: {}",
            app.tr("account.showing"),
            app.settings.owners_line()
        );
    }
    resp.on_hover_text(hover);
    right = acct.left() - 10.0;
    // The tabs, each with its digit and label. On a narrow window (or a large
    // zoom) the name goes first, then the labels of the tabs not shown, so
    // the bell always has room right of them.
    let tabs: Vec<_> = View::ALL
        .iter()
        .map(|&v| {
            let digit = ui.fonts_mut(|f| f.layout_no_wrap(v.digit().to_string(), FontId::monospace(11.0), c.dim));
            let text = ui.fonts_mut(|f| f.layout_no_wrap(app.tr(v.label()).to_owned(), FontId::proportional(13.0), c.fg));
            (v, digit, text)
        })
        .collect();
    let width = |digit: &std::sync::Arc<egui::Galley>, text: Option<&std::sync::Arc<egui::Galley>>| {
        10.0 + digit.size().x + text.map_or(0.0, |t| 6.0 + t.size().x) + 10.0
    };
    let full: f32 = tabs.iter().map(|(_, d, t)| width(d, Some(t)) + 2.0).sum();
    let short: f32 = tabs.iter().map(|(v, d, t)| width(d, app.is_on(*v).then_some(t)) + 2.0).sum();
    let mut x = r.left() + 12.0;
    // The logo, then the name (no kanji: the mark and the word only).
    let logo = Rect::from_center_size(pos2(x + 11.0, mid), vec2(22.0, 22.0));
    if let Some(tex) = logo_texture(&ctx) {
        p.image(tex.id(), logo, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    x = logo.right() + 8.0;
    let name = ui.fonts_mut(|f| f.layout_no_wrap("minato".into(), FontId::proportional(14.0), c.fg));
    let room = right - 34.0;
    if x + name.size().x + 18.0 + full <= room {
        let w = name.size().x;
        p.galley(pos2(x, mid - name.size().y / 2.0), name, c.fg);
        x += w + 18.0;
    } else {
        x += 4.0;
    }
    let labels = x + full <= room || x + short > room;
    // A lane in trouble lights a dot on the status tab, seen from any view.
    let now = crate::app::now_epoch();
    let lane_bad = minato_core::overview::lanes(&app.items).iter().any(|l| l.health(now) == minato_core::overview::Health::Bad);
    for (v, digit, text) in tabs {
        let on = app.is_on(v);
        let text = (labels || on).then_some(text);
        let tab = Rect::from_min_size(pos2(x, mid - 14.0), vec2(width(&digit, text.as_ref()), 28.0));
        let resp = ui.interact(tab, ui.id().with(("tab", v.digit())), Sense::click());
        if on {
            p.rect_filled(tab, CornerRadius::same(6), c.panel);
            p.line_segment([tab.left_bottom() + vec2(8.0, 0.0), tab.right_bottom() - vec2(8.0, 0.0)], Stroke::new(2.0, ACCENT));
        } else if resp.hovered() {
            p.rect_filled(tab, CornerRadius::same(6), c.panel.gamma_multiply(0.6));
        }
        let (dc, tc) = if on { (ACCENT, c.fg) } else { (c.dim, c.dim) };
        p.galley(pos2(tab.left() + 10.0, mid - digit.size().y / 2.0), digit.clone(), dc);
        if let Some(text) = text {
            p.galley(pos2(tab.left() + 10.0 + digit.size().x + 6.0, mid - text.size().y / 2.0), text, tc);
        }
        if v == View::Status && lane_bad {
            crate::style::dot(&p, pos2(tab.right() - 5.0, tab.top() + 6.0), 3.0, crate::style::BAD);
        }
        if resp.clicked() {
            out.view = Some(v);
        }
        if resp.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        if !labels && !on {
            resp.on_hover_text(app.tr(v.label()));
        }
        x = tab.right() + 2.0;
    }
    let left_end = x;
    // The palette field in the middle of what is left, the bell just right
    // of it as in tsumugi.
    let free = Rect::from_min_max(pos2(left_end + 12.0, r.top()), pos2(right - 34.0, r.bottom()));
    let w = (free.width()).clamp(0.0, 420.0);
    let mut bell_left = (right - 28.0).max(left_end + 6.0);
    if w > 120.0 {
        let field = Rect::from_center_size(pos2(r.center().x.clamp(free.left() + w / 2.0, free.right() - w / 2.0), mid), vec2(w, 26.0));
        let resp = ui.interact(field, ui.id().with("palette"), Sense::click());
        p.rect_filled(field, CornerRadius::same(7), if resp.hovered() { c.panel } else { c.side });
        p.rect_stroke(field, CornerRadius::same(7), Stroke::new(1.0, c.border), StrokeKind::Inside);
        // The key goes first when both do not fit: the name says what it is.
        let hint = p.layout_no_wrap(app.tr("palette.hint").to_string(), FontId::proportional(12.5), c.dim);
        let key = p.layout_no_wrap("Ctrl+Shift+P".into(), FontId::monospace(11.0), c.dim);
        let p = p.with_clip_rect(field.shrink(1.0));
        p.galley(pos2(field.left() + 10.0, mid - hint.size().y / 2.0), hint.clone(), c.dim);
        if hint.size().x + key.size().x + 36.0 <= field.width() {
            p.galley(pos2(field.right() - 10.0 - key.size().x, mid - key.size().y / 2.0), key, c.dim);
        }
        if resp.clicked() {
            out.palette = true;
        }
        bell_left = field.right() + 6.0;
    }
    // The bell with the unread count, as in tsumugi: 28 by 24, the cup dim
    // while nothing is unread, a gold badge ("9+" past nine) when something is.
    let bell = Rect::from_min_size(pos2(bell_left, mid - 12.0), vec2(28.0, 24.0));
    let resp = ui.interact(bell, ui.id().with("bell"), Sense::click());
    if resp.hovered() {
        p.rect_filled(bell, CornerRadius::same(6), c.panel);
    }
    draw_bell(&p, bell.center() + vec2(0.0, -1.0), if app.unread == 0 { c.dim } else { c.fg });
    if app.unread > 0 {
        let at = bell.right_top() + vec2(-5.0, 5.0);
        p.circle_filled(at, 7.0, c.wait);
        let text = if app.unread > 9 { "9+".to_string() } else { app.unread.to_string() };
        p.text(at, Align2::CENTER_CENTER, text, FontId::proportional(10.0), c.bg);
    }
    if resp.clicked() {
        out.bell = true;
    }
    out.bell_at = Some(bell);
    resp.on_hover_text(app.tr("bell"));
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

fn draw_bell(p: &egui::Painter, c: Pos2, ink: Color32) {
    let stroke = Stroke::new(1.4, ink);
    let cup = vec![
        c + vec2(-5.5, 4.0),
        c + vec2(-4.5, -1.0),
        c + vec2(-3.0, -4.5),
        c + vec2(0.0, -5.5),
        c + vec2(3.0, -4.5),
        c + vec2(4.5, -1.0),
        c + vec2(5.5, 4.0),
    ];
    p.add(egui::Shape::line(cup, stroke));
    p.line_segment([c + vec2(-6.5, 4.0), c + vec2(6.5, 4.0)], stroke);
    p.circle_filled(c + vec2(0.0, 6.2), 1.4, ink);
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
fn key_bar(ui: &mut Ui, app: &App, r: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let p = ui.painter().clone();
    let mid = r.center().y;
    let mut x = r.left() + 12.0;
    let mut keys = app.bar_keys();
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
    // While F5's sync runs, how many of its answers have come (lit).
    let (sync, ink, widest) = match app.syncing {
        Some((done, total)) => {
            let ja = app.ja();
            (minato_core::status::fetching(done, total, ja), ACCENT, minato_core::status::fetching(total, total, ja))
        }
        None => (minato_core::status::ago(app.since_sync(), app.ja()), c.dim, minato_core::status::ago(Some(59), app.ja())),
    };
    let t = p.text(pos2(right, mid), Align2::RIGHT_CENTER, format!("⟳ {sync}"), FontId::monospace(11.5), ink);
    if app.synced.is_some() {
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
    }
    // The keys stop short of it, kept as wide as two digits (or the sync's
    // total) so they do not come and go as the seconds tick: the last ones
    // go first, then the word of the way out (Esc, or F1), back when it is
    // the only one; its key stays.
    let widest = ui.fonts_mut(|f| f.layout_no_wrap(format!("⟳ {widest}"), FontId::monospace(11.5), c.dim).size().x);
    let stop = right - t.width().max(widest) - 16.0;
    if app.find_on() {
        x = find_bar(ui, app, r, stop, out);
    }
    let wide = |(key, word): &(String, &'static str)| {
        let k = ui.fonts_mut(|f| f.layout_no_wrap(key.clone(), FontId::monospace(11.0), c.fg).size().x);
        let w = ui.fonts_mut(|f| f.layout_no_wrap(word_for(app, word).to_owned(), FontId::proportional(12.0), c.fg).size().x);
        k + 12.0 + 6.0 + w + 14.0
    };
    let mut widths: Vec<f32> = keys.iter().map(wide).collect();
    let mut bare = false;
    while keys.len() > 1 && x + widths.iter().sum::<f32>() - 14.0 > stop {
        let out = keys.iter().rposition(way_out);
        if let Some(o) = out.filter(|_| keys.len() == 2 && !bare) {
            let k = ui.fonts_mut(|f| f.layout_no_wrap(keys[o].0.clone(), FontId::monospace(11.0), c.fg).size().x);
            widths[o] = k + 12.0 + 14.0;
            bare = true;
            continue;
        }
        let at = (0..keys.len()).rev().find(|&k| Some(k) != out).unwrap_or(0);
        keys.remove(at);
        widths.remove(at);
    }
    // Left alone, the way out has its word again where it fits.
    if bare && keys.len() == 1 && x + wide(&keys[0]) - 14.0 <= stop {
        bare = false;
    }
    let out = keys.iter().rposition(way_out);
    for (k, (key, word)) in keys.iter().enumerate() {
        let best = k == 0;
        let kc = if best { ACCENT } else { c.fg };
        let galley = ui.fonts_mut(|f| f.layout_no_wrap(key.clone(), FontId::monospace(11.0), kc));
        let chip = Rect::from_min_size(pos2(x, mid - 9.0), vec2(galley.size().x + 12.0, 18.0));
        p.rect_filled(chip, CornerRadius::same(4), if best { ACCENT.gamma_multiply(0.18) } else { c.panel });
        p.rect_stroke(
            chip,
            CornerRadius::same(4),
            Stroke::new(1.0, if best { ACCENT.gamma_multiply(0.6) } else { c.border }),
            StrokeKind::Inside,
        );
        p.galley(chip.min + vec2(6.0, 9.0 - galley.size().y / 2.0), galley, kc);
        if bare && Some(k) == out {
            x = chip.right() + 14.0;
            continue;
        }
        let label = word_for(app, word);
        let t =
            p.text(pos2(chip.right() + 6.0, mid), Align2::LEFT_CENTER, label, FontId::proportional(12.0), if best { c.fg } else { c.dim });
        x = t.right() + 14.0;
    }
}

/// The key that leaves what is open: Esc, or F1 to the list of keys.
fn way_out(k: &(String, &str)) -> bool {
    k.0.contains("Esc") || k.1 == "key.help"
}

/// The search in the keybar: `/`, its words (the field while it has the
/// keys), how many are left and its keys, short of `stop` (the keys go first,
/// then the field narrows); the x after it.
fn find_bar(ui: &mut Ui, app: &App, r: Rect, stop: f32, out: &mut Out) -> f32 {
    let c = ito_theme::colors();
    let p = ui.painter().clone();
    let mid = r.center().y;
    let x = r.left() + 12.0;
    let galley = ui.fonts_mut(|f| f.layout_no_wrap("/".to_owned(), FontId::monospace(11.0), ACCENT));
    let chip = Rect::from_min_size(pos2(x, mid - 9.0), vec2(galley.size().x + 12.0, 18.0));
    p.rect_filled(chip, CornerRadius::same(4), ACCENT.gamma_multiply(0.18));
    p.rect_stroke(chip, CornerRadius::same(4), Stroke::new(1.0, ACCENT.gamma_multiply(0.6)), StrokeKind::Inside);
    p.galley(chip.min + vec2(6.0, 9.0 - galley.size().y / 2.0), galley, ACCENT);
    let n = app.find_count();
    let hits = ui.fonts_mut(|f| f.layout_no_wrap(format!("{n} {}", app.tr("find.hits")), FontId::monospace(11.5), c.fg).size().x);
    let hint = ui.fonts_mut(|f| f.layout_no_wrap(app.tr("find.keys").to_owned(), FontId::proportional(12.0), c.dim).size().x);
    // The field narrows to keep the keys, then they go and it narrows on.
    let most = 320.0f32.min(r.width() / 3.0);
    let room = stop - chip.right() - 8.0 - 14.0 - hits;
    let show_hint = most.min(room - 14.0 - hint) >= most.min(160.0);
    let fw = if show_hint { most.min(room - 14.0 - hint) } else { most.min(room).max(80.0) };
    let field = Rect::from_min_size(pos2(chip.right() + 8.0, mid - 10.0), vec2(fw, 20.0));
    p.rect_filled(field.expand2(vec2(5.0, 0.0)), CornerRadius::same(4), c.panel);
    p.rect_stroke(
        field.expand2(vec2(5.0, 0.0)),
        CornerRadius::same(4),
        Stroke::new(1.0, if app.find_open { ACCENT.gamma_multiply(0.6) } else { c.border }),
        StrokeKind::Inside,
    );
    if app.find_open {
        out.find_at = Some(field);
    } else {
        let g = crate::style::truncated(&p, app.find_text.clone(), FontId::proportional(12.5), c.fg, field.width());
        p.galley(pos2(field.left(), mid - g.size().y / 2.0), g, c.fg);
    }
    let t = p.text(
        pos2(field.right() + 14.0, mid),
        Align2::LEFT_CENTER,
        format!("{n} {}", app.tr("find.hits")),
        FontId::monospace(11.5),
        if n == 0 { crate::style::BAD } else { c.fg },
    );
    if !show_hint {
        return t.right() + 14.0;
    }
    let t = p.text(pos2(t.right() + 14.0, mid), Align2::LEFT_CENTER, app.tr("find.keys"), FontId::proportional(12.0), c.dim);
    t.right() + 14.0
}

/// The search box over its field in the keybar, while it has the keys: its
/// text is the search's, set as it is typed.
pub fn find_box(ui: &mut Ui, app: &mut App, at: Option<Rect>) {
    let Some(field) = at.filter(|_| app.find_open) else { return };
    let ctx = ui.ctx().clone();
    if std::mem::take(&mut app.find_fresh) {
        // The `/` that opened it is in this frame's input: the field
        // starts with the next.
        ctx.request_repaint();
        return;
    }
    let c = ito_theme::colors();
    let id = egui::Id::new("minato-find");
    let mut text = app.find_text.clone();
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(field));
    let edit = egui::TextEdit::singleline(&mut text)
        .id(id)
        .frame(egui::Frame::NONE)
        .desired_width(field.width())
        .lock_focus(true)
        .font(FontId::proportional(12.5))
        .text_color(c.fg)
        .hint_text(app.tr("find.hint"));
    child.add(edit);
    // Only when it has lost it: asking again ends an IME composition.
    ctx.memory_mut(|m| {
        if !m.has_focus(id) {
            m.request_focus(id)
        }
    });
    if text != app.find_text {
        app.set_find(&text);
    }
}

/// A key's words: a view-key word, or (for the settings' own entries) the
/// settings table's.
fn word_for(app: &App, key: &'static str) -> &'static str {
    app.tr(key)
}

/// The list of every key: the global ones, then the view's; over the inbox,
/// a table of its cards' reasons beside them, below them on a narrower
/// window, or on a second page (Tab) when it would not fit.
fn help(ui: &mut Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let p = ui.painter().clone();
    // Two columns: the keys of everywhere and of the notifications at the
    // left, the view's at the right, so they all fit.
    // The keys in force (keymap.toml), as they are read in the view.
    let view = Ctx::View(app.view);
    let (global, notes, own) =
        (app.key_rows(view, &GLOBAL_KEYS), app.key_rows(Ctx::Notes, &NOTES_KEYS), app.key_rows(view, app.view.keys()));
    let left: [(&str, &[(String, &'static str)]); 2] = [(app.tr("help.global"), &global), (app.tr("help.notes"), &notes)];
    let right: [(&str, &[(String, &'static str)]); 1] = [(app.tr(app.view.label()), &own)];
    let tall = |s: &[(&str, &[(String, &'static str)])]| s.iter().map(|(_, rows)| 28.0 + rows.len() as f32 * 20.0).sum::<f32>();
    let rules = app.view == View::Inbox;
    let rules_h = 48.0 + (RULES.len() + HELP_MARKS.len()) as f32 * 20.0;
    // The table stands as a third column when the window is wide enough,
    // else under the two, else on a page of its own.
    let wide = rules && r.width() - 24.0 >= 1180.0;
    let keys_h = tall(&left).max(tall(&right));
    let paged = rules && !wide && 50.0 + keys_h + rules_h + 30.0 > r.height() - 24.0;
    let (show_keys, below) = match (paged, app.help_rules) {
        (true, true) => (false, true),
        (true, false) => (true, false),
        _ => (true, rules),
    };
    let cols = if show_keys { keys_h } else { 0.0 };
    let h = 50.0 + cols.max(if wide { rules_h } else { 0.0 }) + (if below && !wide { rules_h } else { 0.0 }) + 30.0;
    let w = (r.width() - 24.0).min(if wide { 1300.0 } else { 860.0 });
    let card = Rect::from_center_size(r.center(), vec2(w, h.min(r.height() - 24.0).max(80.0)));
    p.rect_filled(card, CornerRadius::same(10), c.raised());
    p.rect_stroke(card, CornerRadius::same(10), Stroke::new(1.0, c.border_strong()), StrokeKind::Inside);
    p.text(pos2(card.left() + 20.0, card.top() + 18.0), Align2::LEFT_TOP, app.tr("help.title"), FontId::proportional(16.0), c.fg);
    let x0 = card.left() + 20.0;
    // On a short window the rows close up (down to 15) so none is lost.
    let room = card.bottom() - 28.0 - (card.top() + 50.0);
    let pitch = [&left[..], &right[..]]
        .iter()
        .map(|s| {
            let rows = s.iter().map(|(_, rows)| rows.len()).sum::<usize>().max(1) as f32;
            (room - s.len() as f32 * 28.0) / rows
        })
        .fold(20.0f32, f32::min)
        .max(15.0);
    let (left_w, right_w) = if wide { (330.0, 280.0) } else { ((w - 40.0) / 2.0, (w - 40.0) / 2.0) };
    // A column from its `top`-th key row (`j` `k` move it when even the
    // closest rows do not fit): the rows skipped and those left out.
    let bottom = card.bottom() - 28.0;
    let column = |x: f32, col_w: f32, sections: &[(&str, &[(String, &'static str)])], top: usize| {
        let (mut y, mut skip, mut out) = (card.top() + 50.0, top, 0);
        for (title, rows) in sections {
            let from = skip.min(rows.len());
            skip -= from;
            if from == rows.len() {
                continue;
            }
            if out > 0 || y + 20.0 + pitch.min(18.0) > bottom {
                out += rows.len() - from;
                continue;
            }
            p.text(pos2(x, y), Align2::LEFT_TOP, *title, FontId::proportional(11.5), ACCENT);
            y += 20.0;
            for (key, word) in &rows[from..] {
                if out > 0 || y + pitch.min(18.0) > bottom {
                    out += 1;
                    continue;
                }
                // A long key from keymap.toml is cut before the words.
                p.with_clip_rect(Rect::from_min_max(pos2(x, y), pos2(x + 114.0, y + 20.0))).text(
                    pos2(x, y),
                    Align2::LEFT_TOP,
                    key,
                    FontId::monospace(12.0),
                    c.fg,
                );
                p.with_clip_rect(Rect::from_min_max(pos2(x, y), pos2(x + col_w - 12.0, y + 20.0))).text(
                    pos2(x + 120.0, y),
                    Align2::LEFT_TOP,
                    app.tr(word),
                    FontId::proportional(12.5),
                    c.dim,
                );
                y += pitch;
            }
            y += 8.0;
        }
        (top - skip, out)
    };
    let mut last = 0;
    if show_keys {
        for (skipped, out) in [column(x0, left_w, &left, app.help_top), column(x0 + left_w, right_w, &right, app.help_top)] {
            last = last.max(skipped + out);
        }
    }
    app.help_last.set(last);
    let (x, mut y) = if wide { (x0 + left_w + right_w, card.top() + 50.0) } else { (x0, card.top() + 50.0 + cols) };
    if below && y + 58.0 <= card.bottom() - 28.0 {
        // One row per rule (its key, reason and what to do), then the keys
        // for any card.
        let end = card.right() - 20.0;
        let inbox = Ctx::View(View::Inbox);
        let rows: Vec<_> = RULES
            .iter()
            .map(|rule| (rule.next().0, rule.why(), rule.todo()))
            .chain(HELP_MARKS)
            .map(|(k, why, todo)| (app.char_label(inbox, k), why, todo))
            .collect();
        // The key column is as wide as its widest key (`Ctrl+D` when
        // keymap.toml moves one), at least 40.
        let widest = rows.iter().map(|(k, _, _)| p.layout_no_wrap(k.clone(), FontId::monospace(12.0), c.fg).size().x).fold(0.0, f32::max);
        let key_w = (widest + 12.0).max(40.0);
        let (why_x, do_x) = (x + key_w, x + key_w + (end - x - key_w) * 0.4);
        p.text(pos2(x, y), Align2::LEFT_TOP, app.tr("help.rules"), FontId::proportional(11.5), ACCENT);
        y += 20.0;
        for (at, word) in [(x, "help.key"), (why_x, "help.why"), (do_x, "help.do")] {
            p.text(pos2(at, y), Align2::LEFT_TOP, app.tr(word), FontId::proportional(11.0), c.dim);
        }
        y += 20.0;
        for (key, why, todo) in rows {
            if y + 18.0 > card.bottom() - 28.0 {
                break;
            }
            p.text(pos2(x, y), Align2::LEFT_TOP, key, FontId::monospace(12.0), c.fg);
            p.with_clip_rect(Rect::from_min_max(pos2(why_x, y), pos2(do_x - 12.0, y + 20.0))).text(
                pos2(why_x, y),
                Align2::LEFT_TOP,
                app.tr(why),
                FontId::proportional(12.5),
                c.fg,
            );
            p.with_clip_rect(Rect::from_min_max(pos2(do_x, y), pos2(end, y + 20.0))).text(
                pos2(do_x, y),
                Align2::LEFT_TOP,
                app.tr(todo),
                FontId::proportional(12.5),
                c.dim,
            );
            y += 20.0;
        }
    }
    p.text(pos2(card.left() + 20.0, card.bottom() - 22.0), Align2::LEFT_CENTER, app.tr("help.close"), FontId::proportional(11.5), c.dim);
    let hint = match (paged, last > 0) {
        (true, true) => format!("{}  ·  {}", app.tr("help.scroll"), app.tr("help.page")),
        (true, false) => app.tr("help.page").to_string(),
        (false, true) => app.tr("help.scroll").to_string(),
        (false, false) => String::new(),
    };
    p.text(pos2(card.right() - 20.0, card.bottom() - 22.0), Align2::RIGHT_CENTER, hint, FontId::proportional(11.5), c.dim);
}

/// The keys for any inbox card, after the rules in the help.
const HELP_MARKS: [(&str, &str, &str); 3] =
    [("c", "help.any_card", "do.prompt"), ("d", "help.any_card", "do.done"), ("s", "help.any_card", "do.later")];

/// The keys over the notifications, for the help.
const NOTES_KEYS: [(&str, &str); 7] = [
    ("{down} {up}", "notes.pick"),
    ("{enter}", "key.open"),
    ("{open}", "key.browser"),
    ("{open_outside}", "key.outside"),
    ("{read}", "key.read"),
    ("{read_all}", "notes.read_all"),
    ("{close_notes} Esc", "notes.close"),
];

/// The message, while its time lasts: at the top right under the title bar,
/// over everything (the settings, the boxes, the palette), as in kura; a
/// long one wraps at half the window. Left of `page`, the built-in browser,
/// which is over everything egui draws.
pub fn toast(ctx: &egui::Context, app: &App, page: Option<Rect>) {
    let Some(left) = app.toast_left() else { return };
    let Some((text, _)) = &app.toast else { return };
    let c = ito_theme::colors();
    let screen = ctx.content_rect();
    let wrap = (screen.width() / 2.0 - 28.0).max(120.0);
    let galley = ctx.fonts_mut(|f| f.layout(text.clone(), FontId::proportional(12.5), c.fg, wrap));
    let size = galley.size() + vec2(28.0, 14.0);
    let right = page.map_or(screen.right(), |p| p.left());
    let box_ = Rect::from_min_size(pos2(right - 12.0 - size.x, screen.top() + TITLE_H + 6.0), size);
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("minato-toast")));
    p.rect_filled(box_, CornerRadius::same(8), c.raised());
    p.rect_stroke(box_, CornerRadius::same(8), Stroke::new(1.0, c.border_strong()), StrokeKind::Inside);
    p.galley(box_.min + vec2(14.0, 7.0), galley, c.fg);
    ctx.request_repaint_after(left);
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

    #[test]
    fn the_browser_keeps_room_for_the_view() {
        assert_eq!(browser_width(560.0, 1280.0), 560.0);
        assert_eq!(browser_width(100.0, 1280.0), 320.0);
        assert_eq!(browser_width(1200.0, 1280.0), 980.0);
        // A narrow window: the browser's least comes first.
        assert_eq!(browser_width(560.0, 500.0), 320.0);
    }
}
