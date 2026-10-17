//! minato: a keyboard-driven GitHub client, the sibling of kura, yagura and
//! tsumugi. The window and the input; the state is in `app.rs`, the drawing
//! in `ui.rs` and `settings.rs`, and what has nothing to do with egui in the
//! `minato-core` crate.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod actions_ui;
mod app;
mod board_ui;
mod bugreport;
mod compose_ui;
mod config;
mod detail_ui;
mod fonts;
mod i18n;
mod icon;
mod inbox_ui;
mod keymap;
mod list_ui;
mod md_ui;
mod net;
mod notes_ui;
mod overlay;
mod overlay_ui;
mod prefs;
mod script;
mod security_ui;
mod select;
mod settings;
mod side_ui;
mod status_ui;
mod style;
mod ui;

use app::{App, Mode};
use ito_keyscript::{Do, Runner, Step};
use std::ffi::OsString;

/// The window's icon, in pixels square.
const ICON_PX: u32 = 128;

/// The built-in browser's width when it opens.
const BROWSER_W: f32 = 560.0;

const USAGE: &str = "\
minato — a keyboard-driven GitHub client

usage: minato [--keys SCRIPT] [--version | --help]

  --keys SCRIPT  press these keys once the window is up, as kura's --keys
                 (`jj<Enter>`, `<C-S-p>`, `<Wait:200>`, `<State:name>`, `<Quit>`);
                 MINATO_KEYS_DONE names the file that says how it ended
";

struct Minato {
    app: App,
    net: net::Net,
    /// The built-in browser beside the view (Windows and macOS; Linux opens
    /// the system's, `ito_browser::EMBEDDED`), and where it keeps its
    /// cookies, so the sign-in to GitHub lasts.
    browser: Option<ito_browser::Browser>,
    web_dir: Option<std::path::PathBuf>,
    /// The `--keys` script being played, if any.
    script: Option<Runner>,
    /// What the script asked for in `raw_input_hook`, done in the frame.
    todo: Vec<Do>,
    /// Writes the window's place for the next start (`None` for a script's
    /// window, or when the place is not remembered).
    tracker: Option<ito_common::window::Tracker>,
    /// The `always_on_top` last sent to the window.
    on_top: bool,
    /// What the window looked like in the last frame, for `on_exit`.
    seen: Option<ito_common::window::Seen>,
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

    fn write_state(&self, path: std::path::PathBuf, tail: &str) {
        let text = format!("{}{tail}", self.app.report());
        if let Err(e) = std::fs::write(&path, text) {
            ito_console::err(&format!(
                "minato --keys: {}: {e}
",
                path.display()
            ));
        }
    }

    /// The script's shots and states, and its end.
    fn script(&mut self, ctx: &egui::Context) {
        let Some(script) = &mut self.script else { return };
        if let Some(err) = script.shot(ctx) {
            self.app.say(err);
        }
        for d in std::mem::take(&mut self.todo) {
            let Some(script) = &self.script else { return };
            match d {
                Do::State(name) => self.write_state(script.dir.join(format!("{name}.txt")), ""),
                Do::Quit => self.app.quit = true,
                // minato defines no readings of its own.
                Do::Read { .. } => {}
                Do::Done => {
                    if let Some(done) = script.done.clone() {
                        self.write_state(
                            done,
                            "keys: done
",
                        );
                    }
                }
            }
        }
        if let Some(due) = self.script.as_ref().and_then(Runner::due) {
            ctx.request_repaint_after(due);
        }
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
                // A question asked: Enter answers it, nothing else is read.
                egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } if self.app.confirm.is_some() => {
                    self.app.enter();
                }
                _ if self.app.confirm.is_some() => {}
                // egui hands Ctrl+C, Ctrl+X and Ctrl+V over as these, not as
                // keys.
                egui::Event::Copy => match select::text() {
                    Some(text) => ctx.copy_text(text),
                    None => _ = self.app.press(ito_keys::Key::ctrl('c')),
                },
                egui::Event::Cut => _ = self.app.press(ito_keys::Key::ctrl('x')),
                egui::Event::Paste(_) => _ = self.app.press(ito_keys::Key::ctrl('v')),
                // The rest through keymap.toml; the characters come as text.
                egui::Event::Key { key, pressed: true, modifiers, .. } => {
                    if let Some(k) = ito_keys::from_egui(key, &modifiers) {
                        self.app.press(k);
                    }
                }
                egui::Event::Text(t) => {
                    let mut chars = t.chars();
                    while let Some(c) = chars.next() {
                        self.app.press(ito_keys::Key::char(c));
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
        hit: ui::ACCENT,
        accent: ui::ACCENT,
    };
    let words = ito_palette::Words { hint: app.tr("palette.hint"), none: app.tr("palette.none"), header: "", footer: "" };
    let entries = app.palette_entries(&view.query);
    match ito_palette::show(ctx, &mut view, &entries, &style, &words) {
        None => app.palette = Some(view),
        // `View::keys` is empty, so `Key` never comes.
        Some(ito_palette::Answer::Close | ito_palette::Answer::Key(..)) => {}
        Some(ito_palette::Answer::Pick(cmd)) => app.run_cmd(cmd),
    }
}

impl eframe::App for Minato {
    /// A window that closes while a script runs says so in the done file
    /// (`<Quit>` and a last key that quits both come here).
    fn on_exit(&mut self) {
        if let Some(t) = &mut self.tracker {
            t.close(self.seen);
        }
        let Some(script) = &mut self.script else { return };
        if !script.running() {
            return;
        }
        let left = script.left();
        script.finish();
        let Some(done) = script.done.clone() else { return };
        let tail = if self.app.quit { ito_keyscript::quit_report(left) } else { ito_keyscript::closed_report(left) };
        self.write_state(done, &tail);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(script) = &mut self.script {
            if let Some(d) = script.input(ctx, raw) {
                self.todo.push(d);
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let place = self.app.prefs.common.window_or_default();
        if place.remember_place {
            if let Some(t) = &mut self.tracker {
                t.follow(&ctx);
            }
            self.seen = ito_common::window::Seen::of(&ctx);
        } else {
            self.tracker = None;
        }
        if place.always_on_top != self.on_top {
            self.on_top = place.always_on_top;
            let level = if self.on_top { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal };
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
        }
        // While the Keys page takes a key, every key goes to it.
        let taking = self.app.mode == Mode::Settings && self.app.prefs.keys_edit.busy();
        if let Some(to) = (!taking).then(|| ctx.input_mut(scale_key)).flatten() {
            self.app.scale(to);
        }
        if !taking && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F1)) {
            self.app.toggle_help();
        }
        if !taking && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F12)) {
            self.app.bug_report();
        }
        if !taking && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F5)) {
            self.app.sync_now();
        }
        if self.app.mode != Mode::Settings {
            self.keys(&ctx);
        }
        self.script(&ctx);
        self.app.prefs.pump();
        match self.app.prefs.keymap_new.take() {
            Some(None) => self.app.load_keys(),
            Some(Some(e)) => self.app.say(e),
            None => {}
        }
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
            // Left the settings while a key was taken: give it up, or the
            // page comes back still taking keys (Esc included).
            if self.app.prefs.keys_edit.busy() {
                self.app.prefs.keys_edit.capture = None;
                self.app.prefs.keys_edit.listening = false;
            }
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
            // A press outside the strip the box is in lets go of it too, on
            // blank ground and on the detail's text as well.
            let pressed_away = out.compose_at.is_some()
                && self.app.compose.as_ref().is_some_and(|c| c.picker.is_none())
                && ctx.input(|i| i.pointer.primary_pressed())
                && ctx.input(|i| i.pointer.press_origin()).zip(out.strip).is_some_and(|(o, s)| !s.contains(o));
            if inline && (elsewhere || pressed_away) {
                self.app.close_compose();
                ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("minato-compose")));
            }
            if let Some(v) = out.view {
                self.app.view = v;
            }
            // A click in the view under the notifications only closes them.
            if let Some(c) = out.click.filter(|_| was == Mode::Main) {
                self.app.click(c);
            }
            if let Some(u) = out.open_url.filter(|_| was == Mode::Main) {
                self.app.open_url = Some(u);
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

/// Puts a line where whoever typed the command is looking, on standard error
/// when `err` (kura's `say`).
///
/// A release build on Windows is a GUI-subsystem program and has no console
/// of its own; `ito_console` writes to a file or a pipe when the standard
/// handle is one (`minato.com` hands on its own, `cmd`'s `>`), and otherwise
/// to the console minato was started from. A write that fails is let go:
/// PowerShell gives a bare `$v = & minato.exe --version` a pipe and closes it
/// without waiting, and `println!` panicked on that ("failed printing to
/// stdout", os error 232). `minato.com` is the way to read it there.
fn say(text: &str, err: bool) {
    let line = format!(
        "{text}
"
    );
    match err {
        true => ito_console::err(&line),
        false => ito_console::out(&line),
    }
}

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
enum Cli {
    Version,
    Help,
    /// Open the window, playing this script if there is one.
    Open(Option<Vec<Step>>),
}

fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Cli, String> {
    let mut script = None;
    let mut args = args.into_iter();
    while let Some(a) = args.next() {
        match a.to_str() {
            Some("--keys") => {
                let s = args.next().ok_or("--keys needs a script")?;
                let s = s.to_str().ok_or("--keys: the script is not text")?;
                script = Some(ito_keyscript::parse(s).map_err(|e| format!("--keys: {e}"))?);
            }
            Some("--version" | "-V") => return Ok(Cli::Version),
            Some("--help" | "-h") => return Ok(Cli::Help),
            _ => return Err(format!("unknown argument `{}`", a.to_string_lossy())),
        }
    }
    Ok(Cli::Open(script))
}

fn main() -> eframe::Result {
    let script = match parse_args(std::env::args_os().skip(1)) {
        Ok(Cli::Version) => {
            say(&format!("minato {}", env!("CARGO_PKG_VERSION")), false);
            return Ok(());
        }
        Ok(Cli::Help) => {
            say(USAGE, false);
            return Ok(());
        }
        Ok(Cli::Open(script)) => script,
        Err(e) => {
            say(
                &format!(
                    "minato: {e}

{USAGE}"
                ),
                true,
            );
            std::process::exit(2);
        }
    };
    let done = std::env::var_os("MINATO_KEYS_DONE").filter(|v| !v.is_empty()).map(std::path::PathBuf::from);
    if let (Some(_), Some(done)) = (&script, &done) {
        ito_keyscript::install_panic_report(done.clone());
    }
    // A window a script drives, or one told not to, keeps no place.
    let first = ito_common::base_dir().and_then(|b| ito_common::Common::read(&b).ok()).unwrap_or_default().0.window_or_default();
    let saved =
        ito_common::window::Saved::load(if script.is_none() && first.remember_place { ito_common::state_dir("minato") } else { None });
    let viewport = egui::ViewportBuilder::default()
        .with_title("minato")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([720.0, 420.0])
        // The frame is drawn by ui.rs: the title bar, the buttons, the edges.
        .with_decorations(false)
        // Alt+Tab and the taskbar; build.rs bakes the same drawing into
        // minato.exe for Explorer.
        .with_icon(std::sync::Arc::new(egui::IconData { rgba: icon::pixels(ICON_PX), width: ICON_PX, height: ICON_PX }));
    let viewport = saved.viewport(viewport);
    let viewport = if first.always_on_top { viewport.with_always_on_top() } else { viewport };
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "minato",
        options,
        Box::new(move |cc| {
            saved.put(cc);
            fonts::load_in_background(cc.egui_ctx.clone());
            let mut prefs = prefs::Prefs::load(ito_common::base_dir());
            prefs.watch(&cc.egui_ctx);
            prefs.apply_theme(&cc.egui_ctx);
            // The scale is common.toml's, so egui's own Ctrl+= / Ctrl+- stay out of it.
            cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
            cc.egui_ctx.set_zoom_factor(prefs.common.scale_or_one());
            let (app_dir, state) = (ito_common::app_dir("minato"), ito_common::state_dir("minato"));
            // The files of before v0.61.0 move to config.toml and the state folder first.
            let moved = match (&prefs.base, &app_dir) {
                (Some(b), Some(a)) => config::migrate(b, a, state.as_deref()),
                _ => None,
            };
            let marks_file = state.as_ref().map(|d| config::marks_path(d));
            let settings_file = app_dir.as_ref().map(|d| config::path(d));
            let state_file = state.as_ref().map(|d| d.join("state.toml"));
            let seen_file = state.as_ref().map(|d| d.join("seen-runs.txt"));
            let mut app = App::new(prefs);
            if let Some(f) = marks_file {
                app.keep_marks(f);
            }
            if let Some(f) = settings_file {
                app.keep_settings(f);
            }
            if let Some(f) = state_file {
                app.keep_state(f);
            }
            if let Some(f) = seen_file {
                app.keep_seen(f);
            }
            if let Some(files) = moved {
                let line = app.tr("settings.moved").replacen("{}", &files, 1);
                app.say(line);
            }
            let net = net::Net::start(&cc.egui_ctx, ito_common::state_dir("minato").map(|d| d.join("cache")));
            let script = script.map(|steps| {
                let mut r = Runner::new(steps, done);
                r.watch(cc.egui_ctx.clone());
                r
            });
            let web_dir = ito_common::state_dir("minato").map(|d| d.join("webview"));
            Ok(Box::new(Minato {
                app,
                net,
                browser: None,
                web_dir,
                script,
                todo: Vec::new(),
                tracker: saved.tracker(),
                on_top: first.always_on_top,
                seen: None,
            }))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, String> {
        parse_args(args.iter().map(OsString::from))
    }

    #[test]
    fn the_command_line() {
        assert_eq!(parse(&[]), Ok(Cli::Open(None)));
        assert_eq!(parse(&["--version"]), Ok(Cli::Version));
        assert_eq!(parse(&["-V"]), Ok(Cli::Version));
        assert_eq!(parse(&["x", "-h"]), Err("unknown argument `x`".to_owned()));
        assert_eq!(parse(&["-h"]), Ok(Cli::Help));
        assert!(matches!(parse(&["--keys", "j<Enter>"]), Ok(Cli::Open(Some(s))) if !s.is_empty()));
        assert!(parse(&["--keys"]).is_err());
        assert!(parse(&["--keys", "<Bogus>"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
    }

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
}
