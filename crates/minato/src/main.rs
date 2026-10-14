//! minato: a keyboard-driven GitHub client, the sibling of kura, yagura and
//! tsumugi. The window and the input; the state is in `app.rs`, the drawing
//! in `ui.rs` and `settings.rs`, and what has nothing to do with egui in the
//! `minato-core` crate.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod actions_ui;
mod app;
mod board_ui;
mod compose_ui;
mod detail_ui;
mod fonts;
mod i18n;
mod icon;
mod inbox_ui;
mod list_ui;
mod md_ui;
mod net;
mod notes_ui;
mod overlay;
mod overlay_ui;
mod prefs;
mod settings;
mod style;
mod ui;

use app::{App, Mode};

/// The window's icon, in pixels square.
const ICON_PX: u32 = 128;

/// The built-in browser's width when it opens.
const BROWSER_W: f32 = 560.0;

const USAGE: &str = "\
minato — a keyboard-driven GitHub client

usage: minato [--version | --help]
";

struct Minato {
    app: App,
    net: net::Net,
    /// The built-in browser beside the view (Windows and macOS; Linux opens
    /// the system's, `ito_browser::EMBEDDED`), and where it keeps its
    /// cookies, so the sign-in to GitHub lasts.
    browser: Option<ito_browser::Browser>,
    web_dir: Option<std::path::PathBuf>,
}

impl Minato {
    /// A page beside the view, in the browser there or a new one.
    fn show_page(&mut self, ctx: &egui::Context, url: String) {
        match &mut self.browser {
            Some(b) => b.go(url),
            None => self.browser = Some(ito_browser::Browser::new(url).with_data_dir(self.web_dir.clone())),
        }
        self.app.browser_w.get_or_insert(BROWSER_W);
        // The notifications would hide it.
        if self.app.mode == Mode::Notes {
            self.app.mode = Mode::Main;
        }
        ctx.request_repaint();
    }

    fn close_browser(&mut self) {
        self.browser = None;
        self.app.browser_w = None;
    }

    /// The browser in `at`, or out of sight where the frame left it no room
    /// (the settings, the help). A view of the system is over all egui
    /// draws, so it is hidden while a box is over the view.
    fn browser_ui(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame, at: Option<egui::Rect>) {
        let Some(b) = &mut self.browser else { return };
        let Some(at) = at else {
            b.hide();
            return;
        };
        let app = &self.app;
        let hidden = app.mode == Mode::Notes
            || app.palette.is_some()
            || app.overlay.is_some()
            || app.confirm.is_some()
            || app.compose.as_ref().is_some_and(|c| c.id.starts_with("new:"));
        ui.painter().rect_filled(at, 0.0, ito_theme::colors().side);
        // The right 6 px are the window's edge to resize by.
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(at.shrink(6.0)));
        let words = ito_browser::Words {
            back: app.tr("browser.back"),
            reload: app.tr("browser.reload"),
            external: app.tr("browser.external"),
            close: app.tr("browser.close"),
            failed: app.tr("browser.failed"),
        };
        let answer = b.show(&mut child, frame, hidden, &words);
        if let Some(url) = answer.external {
            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
        }
        if answer.close {
            self.close_browser();
        }
    }

    /// The characters typed and Escape, when no text field has the keys.
    fn keys(&mut self, ctx: &egui::Context) {
        let (events, palette, settings) = ctx.input_mut(|i| {
            let palette = i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::P);
            let settings = i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma);
            (i.events.clone(), palette, settings)
        });
        let events = no_alt_letters(events);
        if settings && self.app.compose.is_none() && self.app.confirm.is_none() && self.app.overlay.is_none() {
            self.app.palette = None;
            self.app.mode = Mode::Settings;
        }
        if palette && self.app.compose.is_none() && self.app.confirm.is_none() {
            self.app.palette = self.app.palette.is_none().then(ito_palette::View::new);
        }
        for e in events {
            if self.app.compose.is_some() || self.app.palette.is_some() {
                break;
            }
            if self.app.overlay.is_some() {
                for k in overlay_keys(&e) {
                    self.app.overlay_key(k);
                }
                continue;
            }
            // The search box takes the text; Enter, Esc and the arrows are
            // read here.
            if self.app.find_open {
                match e {
                    egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => self.app.find_clear(),
                    egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } => self.app.find_open = false,
                    egui::Event::Key { key: egui::Key::ArrowDown, pressed: true, .. } => _ = self.app.feed('j'),
                    egui::Event::Key { key: egui::Key::ArrowUp, pressed: true, .. } => _ = self.app.feed('k'),
                    _ => {}
                }
                continue;
            }
            match e {
                egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => {
                    // With nothing else to close, the browser.
                    if !self.app.escape() && self.browser.is_some() {
                        self.close_browser();
                    }
                }
                egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } => {
                    self.app.enter();
                }
                egui::Event::Key { key: egui::Key::Tab, pressed: true, modifiers, .. }
                    if !modifiers.ctrl && !modifiers.alt && !modifiers.command =>
                {
                    self.app.cycle_view(modifiers.shift);
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } => {
                    if let Some(c) = arrow_char(key, &modifiers) {
                        self.app.feed(c);
                    }
                }
                egui::Event::Text(t) => {
                    let mut chars = t.chars();
                    while let Some(c) = chars.next() {
                        self.app.feed(c);
                        // Typed fast after `/`: the rest is the search's.
                        if self.app.find_open {
                            let text = format!("{}{}", self.app.find_text, chars.as_str());
                            self.app.set_find(&text);
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// The events without the letters an Alt key also sends on Windows: `Alt+A`
/// is not `a` (AltGr, which comes with Ctrl, still types).
fn no_alt_letters(events: Vec<egui::Event>) -> Vec<egui::Event> {
    let alt: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            egui::Event::Key { key, pressed: true, modifiers, .. } if modifiers.alt && !modifiers.ctrl && key.name().len() == 1 => {
                Some(key.name().to_ascii_lowercase())
            }
            _ => None,
        })
        .collect();
    events.into_iter().filter(|e| !matches!(e, egui::Event::Text(t) if alt.contains(&t.to_ascii_lowercase()))).collect()
}

/// The palette over the window, while it is open.
fn palette_show(ctx: &egui::Context, app: &mut App) {
    let Some(mut view) = app.palette.take() else { return };
    let c = ito_theme::colors();
    let style = ito_palette::Style {
        panel: c.panel,
        border: c.border,
        text: c.fg,
        dim: c.dim,
        selected_bg: ui::ACCENT.gamma_multiply(0.22),
        selected_text: c.fg,
    };
    let words = ito_palette::Words { hint: app.tr("palette.hint"), none: app.tr("palette.none") };
    let entries = app.palette_entries(&view.query);
    match ito_palette::show(ctx, &mut view, &entries, &style, &words) {
        None => app.palette = Some(view),
        Some(ito_palette::Answer::Close) => {}
        Some(ito_palette::Answer::Pick(cmd)) => app.run_cmd(cmd),
    }
}

impl eframe::App for Minato {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(to) = ctx.input_mut(scale_key) {
            self.app.scale(to);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F1)) {
            self.app.toggle_help();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F5)) {
            self.app.sync_now();
        }
        if self.app.mode != Mode::Settings {
            self.keys(&ctx);
        }
        self.app.prefs.pump();
        self.net.pump(&mut self.app, &ctx);
        self.app.set_language();
        // common.toml's theme, or the OS's light or dark under `system`.
        self.app.prefs.apply_theme(&ctx);
        ctx.set_zoom_factor(self.app.prefs.common.scale_or_one());
        if let Some(e) = self.app.prefs.failed.take() {
            self.app.say(e);
        }
        let mut page = None;
        if self.app.mode == Mode::Settings {
            self.app.settle_runs(ctx.input(|i| i.time));
            settings::show(ui, &mut self.app);
            self.browser_ui(ui, frame, None);
        } else {
            let was = self.app.mode;
            self.app.find_check();
            // After this frame's keys, before the runs are drawn.
            self.app.settle_runs(ctx.input(|i| i.time));
            // A box over the view keeps the cards' peeks from showing over it.
            let boxed = self.app.palette.is_some()
                || self.app.overlay.is_some()
                || self.app.confirm.is_some()
                || self.app.compose.as_ref().is_some_and(|c| c.id.starts_with("new:"));
            ui.ctx().data_mut(|d| d.insert_temp(ui::BOXED.into(), boxed));
            let out = ui::frame(ui, &self.app);
            self.app.detail_max = out.detail_max;
            self.app.clamp_scroll();
            page = out.browser_at;
            if let Some(w) = out.browser_w {
                self.app.browser_w = Some(ui::browser_width(w, ui.max_rect().width()));
            }
            // The box in the detail is left (its text kept as the item's
            // draft) for anything else the mouse does.
            let inline = self.app.compose.as_ref().is_some_and(|c| !c.id.starts_with("new:"));
            let elsewhere = out.view.is_some()
                || out.click.as_ref().is_some_and(|c| !c.is_compose())
                || out.note.is_some()
                || out.pick.is_some()
                || out.drop.is_some()
                || out.bell
                || out.palette;
            if inline && elsewhere {
                self.app.close_compose();
            }
            if let Some(v) = out.view {
                self.app.view = v;
            }
            // A click in the view under the notifications only closes them.
            if let Some(c) = out.click.filter(|_| was == Mode::Main) {
                self.app.click(c);
            }
            if let Some(n) = out.split {
                self.app.set_split(self.app.view, n);
            }
            if let Some(n) = out.note {
                self.app.note_click(n);
            }
            if let Some(k) = out.pick {
                self.app.pick_card(k);
            }
            if let Some((k, col)) = out.drop {
                self.app.drop_card(k, col);
            }
            self.app.peek_wanted = out.peek;
            // Not under a box that is over the view.
            if let Some((at, n)) = out.wheel.filter(|_| !boxed) {
                self.app.wheel(at, n, out.detail_max);
            }
            if out.bell {
                self.app.mode = if was == Mode::Notes { Mode::Main } else { Mode::Notes };
            }
            if out.palette && self.app.palette.is_none() {
                self.app.palette = Some(ito_palette::View::new());
            }
            // Before the boxes over the view: its ground would cover them.
            self.browser_ui(ui, frame, page);
            compose_ui::show(ui, &mut self.app, out.compose_at);
            ui::find_box(ui, &mut self.app, out.find_at);
            overlay_ui::show(ui, &self.app);
            palette_show(&ctx, &mut self.app);
        }
        ui::toast(&ctx, &self.app, page.filter(|_| self.browser.is_some()));
        if let Some(text) = self.app.copy.take() {
            ctx.copy_text(text);
        }
        if let Some(url) = self.app.open_url.take() {
            if std::mem::take(&mut self.app.open_outside) || !ito_browser::EMBEDDED {
                ctx.open_url(egui::OpenUrl::new_tab(url));
            } else {
                self.show_page(&ctx, url);
            }
        }
        if self.app.quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ui::resize_edges(&ctx);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        ito_theme::colors().bg.to_normalized_gamma_f32()
    }
}

/// Ctrl+= (or Ctrl++), Ctrl+- and Ctrl+0: the common scale, taken out of
/// the input so a text field does not get them. Shift is ignored, so `+` on
/// a keyboard that needs it for the key still counts.
fn scale_key(i: &mut egui::InputState) -> Option<ito_common::ScaleTo> {
    use ito_common::ScaleTo;
    let keys = [
        (egui::Key::Plus, ScaleTo::In),
        (egui::Key::Equals, ScaleTo::In),
        (egui::Key::Minus, ScaleTo::Out),
        (egui::Key::Num0, ScaleTo::Reset),
    ];
    let mut got = None;
    for (key, to) in keys {
        for m in [egui::Modifiers::COMMAND, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT] {
            if i.consume_key(m, key) {
                got = Some(to);
            }
        }
    }
    got
}

/// The arrows as the letters they stand for (`←↓↑→` are `h j k l`, with
/// Shift `H J K L`), read through ito-keys; none with Ctrl or Alt held.
fn arrow_char(key: egui::Key, mods: &egui::Modifiers) -> Option<char> {
    use ito_keys::{Code, Named};
    let k = ito_keys::from_egui(key, mods)?;
    if k.ctrl || k.alt {
        return None;
    }
    let c = match k.code {
        Code::Named(Named::Left) => 'h',
        Code::Named(Named::Down) => 'j',
        Code::Named(Named::Up) => 'k',
        Code::Named(Named::Right) => 'l',
        _ => return None,
    };
    Some(if k.shift { c.to_ascii_uppercase() } else { c })
}

/// An input event as the Actions boxes see it.
fn overlay_keys(e: &egui::Event) -> Vec<overlay::K> {
    use egui::Key;
    use overlay::K;
    match e {
        egui::Event::Key { key, pressed: true, modifiers, .. } => vec![match key {
            Key::Escape => K::Esc,
            Key::Enter if modifiers.command => K::Send,
            Key::Enter => K::Enter,
            Key::ArrowUp => K::Up,
            Key::ArrowDown => K::Down,
            Key::ArrowLeft => K::Left,
            Key::ArrowRight => K::Right,
            Key::PageUp => K::PageUp,
            Key::PageDown => K::PageDown,
            Key::Backspace => K::Back,
            Key::Tab => K::Down,
            _ => return Vec::new(),
        }],
        egui::Event::Text(t) => t.chars().map(K::Char).collect(),
        _ => Vec::new(),
    }
}

/// A release build on Windows has no console of its own: borrow the one it
/// was started from, so `--help` and `--version` can be read.
#[cfg(windows)]
fn attach_console() {
    // SAFETY: no arguments to get wrong; failing (no parent console) is fine.
    unsafe extern "system" {
        fn AttachConsole(process: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

#[cfg(not(windows))]
fn attach_console() {}

fn main() -> eframe::Result {
    let mut args = std::env::args().skip(1);
    if let Some(a) = args.next() {
        attach_console();
        match a.as_str() {
            "--version" | "-V" => println!("minato {}", env!("CARGO_PKG_VERSION")),
            "--help" | "-h" => println!("{USAGE}"),
            other => {
                eprintln!("minato: unknown argument `{other}`\n\n{USAGE}");
                std::process::exit(2);
            }
        }
        return Ok(());
    }
    let viewport = egui::ViewportBuilder::default()
        .with_title("minato")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([720.0, 420.0])
        // The frame is drawn by ui.rs: the title bar, the buttons, the edges.
        .with_decorations(false)
        // Alt+Tab and the taskbar; build.rs bakes the same drawing into
        // minato.exe for Explorer.
        .with_icon(std::sync::Arc::new(egui::IconData { rgba: icon::pixels(ICON_PX), width: ICON_PX, height: ICON_PX }));
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "minato",
        options,
        Box::new(|cc| {
            fonts::load_in_background(cc.egui_ctx.clone());
            let mut prefs = prefs::Prefs::load(ito_common::base_dir());
            prefs.watch(&cc.egui_ctx);
            prefs.apply_theme(&cc.egui_ctx);
            // The scale is common.toml's, so egui's own Ctrl+= / Ctrl+- stay out of it.
            cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
            cc.egui_ctx.set_zoom_factor(prefs.common.scale_or_one());
            let marks_file = prefs.base.as_ref().map(|b| b.join("minato-marks.json"));
            let settings_file = prefs.base.as_ref().map(|b| b.join("minato-settings.json"));
            let searches_file = prefs.base.as_ref().map(|b| b.join("minato-searches.txt"));
            let state_file = ito_common::state_dir("minato").map(|d| d.join("state.toml"));
            let seen_file = ito_common::state_dir("minato").map(|d| d.join("seen-runs.txt"));
            let mut app = App::new(prefs);
            if let Some(f) = marks_file {
                app.keep_marks(f);
            }
            if let Some(f) = settings_file {
                app.keep_settings(f);
            }
            if let Some(f) = searches_file {
                app.keep_searches(f);
            }
            if let Some(f) = state_file {
                app.keep_state(f);
            }
            if let Some(f) = seen_file {
                app.keep_seen(f);
            }
            let net = net::Net::start(&cc.egui_ctx, ito_common::state_dir("minato").map(|d| d.join("cache")));
            Ok(Box::new(Minato { app, net, browser: None, web_dir: ito_common::state_dir("minato").map(|d| d.join("webview")) }))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_alt_key_does_not_type_its_letter() {
        let key = |key, modifiers| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
        let text = |t: &str| egui::Event::Text(t.to_string());
        let got = no_alt_letters(vec![key(egui::Key::A, egui::Modifiers::ALT), text("a"), text("j")]);
        assert_eq!(got.iter().filter(|e| matches!(e, egui::Event::Text(_))).count(), 1);
        // AltGr comes with Ctrl: its character stays.
        let got = no_alt_letters(vec![key(egui::Key::Q, egui::Modifiers::ALT | egui::Modifiers::CTRL), text("@")]);
        assert_eq!(got.len(), 2);
        assert_eq!(no_alt_letters(vec![text("a")]).len(), 1);
    }

    #[test]
    fn the_arrows_are_hjkl() {
        let none = egui::Modifiers::NONE;
        let got: Vec<_> = [egui::Key::ArrowLeft, egui::Key::ArrowDown, egui::Key::ArrowUp, egui::Key::ArrowRight]
            .into_iter()
            .map(|k| arrow_char(k, &none))
            .collect();
        assert_eq!(got, [Some('h'), Some('j'), Some('k'), Some('l')]);
        assert_eq!(arrow_char(egui::Key::ArrowDown, &egui::Modifiers::SHIFT), Some('J'));
        assert_eq!(arrow_char(egui::Key::ArrowDown, &egui::Modifiers::CTRL), None);
        assert_eq!(arrow_char(egui::Key::ArrowDown, &egui::Modifiers::ALT), None);
        assert_eq!(arrow_char(egui::Key::A, &none), None);
    }
}
