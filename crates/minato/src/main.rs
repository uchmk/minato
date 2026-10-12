//! minato: a keyboard-driven GitHub client, the sibling of kura, yagura and
//! tsumugi. The window and the input; the state is in `app.rs`, the drawing
//! in `ui.rs` and `settings.rs`, and what has nothing to do with egui in the
//! `minato-core` crate.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod actions_ui;
mod board_ui;
mod app;
mod fonts;
mod i18n;
mod compose_ui;
mod detail_ui;
mod icon;
mod inbox_ui;
mod list_ui;
mod notes_ui;
mod net;
mod overlay;
mod overlay_ui;
mod prefs;
mod settings;
mod style;
mod ui;

use app::{App, Mode};

/// The window's icon, in pixels square.
const ICON_PX: u32 = 128;

const USAGE: &str = "\
minato — a keyboard-driven GitHub client

usage: minato [--version | --help]
";

struct Minato {
    app: App,
    net: net::Net,
}

impl Minato {
    /// The characters typed and Escape, when no text field has the keys.
    fn keys(&mut self, ctx: &egui::Context) {
        let (events, palette, settings) = ctx.input_mut(|i| {
            let palette = i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::P);
            let settings = i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma);
            (i.events.clone(), palette, settings)
        });
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
            match e {
                egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => {
                    self.app.escape();
                }
                egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } => {
                    self.app.enter();
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } => {
                    if let Some(c) = arrow_char(key, &modifiers) {
                        self.app.feed(c);
                    }
                }
                egui::Event::Text(t) => {
                    for c in t.chars() {
                        self.app.feed(c);
                    }
                }
                _ => {}
            }
        }
    }
}

/// The palette over the window, while it is open.
fn palette_show(ctx: &egui::Context, app: &mut App) {
    let Some(mut view) = app.palette.take() else { return };
    let c = ito_theme::colors();
    let style = ito_palette::Style { panel: c.panel, border: c.border, text: c.fg, dim: c.dim, selected_bg: ui::ACCENT.gamma_multiply(0.22), selected_text: c.fg };
    let words = ito_palette::Words { hint: app.tr("palette.hint"), none: app.tr("palette.none") };
    match ito_palette::show(ctx, &mut view, &app.palette_entries(), &style, &words) {
        None => app.palette = Some(view),
        Some(ito_palette::Answer::Close) => {}
        Some(ito_palette::Answer::Pick(cmd)) => app.run_cmd(cmd),
    }
}

impl eframe::App for Minato {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
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
        if self.app.mode == Mode::Settings {
            settings::show(ui, &mut self.app);
        } else {
            let was = self.app.mode;
            let out = ui::frame(ui, &self.app);
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
            if out.bell {
                self.app.mode = if was == Mode::Notes { Mode::Main } else { Mode::Notes };
            }
            if out.palette && self.app.palette.is_none() {
                self.app.palette = Some(ito_palette::View::new());
            }
            compose_ui::show(ui, &mut self.app);
            overlay_ui::show(ui, &self.app);
            palette_show(&ctx, &mut self.app);
        }
        if let Some(url) = self.app.open_url.take() {
            ctx.open_url(egui::OpenUrl::new_tab(url));
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
            Ok(Box::new(Minato { app, net: net::Net::start(&cc.egui_ctx) }))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrows_are_hjkl() {
        let none = egui::Modifiers::NONE;
        let got: Vec<_> = [egui::Key::ArrowLeft, egui::Key::ArrowDown, egui::Key::ArrowUp, egui::Key::ArrowRight].into_iter().map(|k| arrow_char(k, &none)).collect();
        assert_eq!(got, [Some('h'), Some('j'), Some('k'), Some('l')]);
        assert_eq!(arrow_char(egui::Key::ArrowDown, &egui::Modifiers::SHIFT), Some('J'));
        assert_eq!(arrow_char(egui::Key::ArrowDown, &egui::Modifiers::CTRL), None);
        assert_eq!(arrow_char(egui::Key::ArrowDown, &egui::Modifiers::ALT), None);
        assert_eq!(arrow_char(egui::Key::A, &none), None);
    }
}
