//! minato: a keyboard-driven GitHub client, the sibling of kura, yagura and
//! tsumugi. The window and the input; the state is in `app.rs`, the drawing
//! in `ui.rs` and `settings.rs`, and what has nothing to do with egui in the
//! `minato-core` crate.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod fonts;
mod i18n;
mod icon;
mod list_ui;
mod net;
mod prefs;
mod settings;
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
        let (events, palette) = ctx.input_mut(|i| {
            let palette = i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::P);
            (i.events.clone(), palette)
        });
        if palette {
            // The palette comes with the commands (stage 6).
            let words = self.app.tr("empty.soon");
            self.app.say(words);
        }
        for e in events {
            match e {
                egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => {
                    self.app.escape();
                }
                egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } => {
                    self.app.enter();
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
            let out = ui::frame(ui, &self.app);
            if let Some(v) = out.view {
                self.app.view = v;
            }
            if out.settings {
                self.app.mode = Mode::Settings;
            }
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
            Ok(Box::new(Minato { app: App::new(prefs), net: net::Net::start(&cc.egui_ctx) }))
        }),
    )
}
