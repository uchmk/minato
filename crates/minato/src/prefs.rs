//! What the settings screen reads and writes (ito's `docs/common-spec.md`):
//! common.toml (the language, the scale, the theme and the clock, shared by
//! every uchmk app) and the user's themes. The files are watched every 2 s
//! and written on threads of their own; what was read comes back as a
//! [`Reload`], which [`Prefs::pump`] folds in. The UI thread never touches
//! the disk.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use ito_common::{common_path, edit_common, themes_dir, write_atomic, Common, CommonChange};
use ito_prefs::State;
use ito_theme::Theme;

/// How often the files are looked at.
const WATCH: Duration = Duration::from_secs(2);

/// A file read again, or a write that failed.
pub enum Reload {
    Common(Result<(Common, Vec<String>), String>),
    Themes(Vec<Theme>, Vec<String>),
    Failed(String),
}

pub struct Prefs {
    /// The `uchmk` folder; `None` when the OS has no config directory.
    pub base: Option<PathBuf>,
    pub common: Common,
    /// The built-in themes, then the user's.
    pub themes: Vec<Theme>,
    pub common_warn: Vec<String>,
    pub theme_warn: Vec<String>,
    /// The theme common.toml names but which is not there.
    pub theme_err: Option<String>,
    /// The screen's page, search and focus.
    pub state: State,
    /// The last write that failed, for a toast.
    pub failed: Option<String>,
    tx: Sender<Reload>,
    rx: Receiver<Reload>,
    wake: Option<egui::Context>,
    /// The scale the keys asked for, until common.toml has it: an older
    /// read coming back meanwhile does not undo it.
    scale_wanted: Option<f32>,
    /// The thread that writes the scale, the newest one of a burst.
    scale_tx: Option<Sender<f32>>,
    /// The OS's light or dark, for `system` (egui's lags a switch on Windows).
    os_light: ito_common::OsLight,
}

impl Prefs {
    /// Nothing read: the defaults and the built-in themes (the tests).
    pub fn empty() -> Self {
        let (tx, rx) = channel();
        Self {
            base: None,
            common: Common::default(),
            themes: ito_theme::builtin(),
            common_warn: Vec::new(),
            theme_warn: Vec::new(),
            theme_err: None,
            state: State::default(),
            failed: None,
            tx,
            rx,
            wake: None,
            scale_wanted: None,
            scale_tx: None,
            os_light: ito_common::OsLight::default(),
        }
    }

    /// common.toml and the themes in `base`.
    pub fn load(base: Option<PathBuf>) -> Self {
        let mut p = Self::empty();
        if let Some(b) = &base {
            match Common::read(b) {
                Ok((c, w)) => (p.common, p.common_warn) = (c, w),
                Err(e) => p.common_warn = vec![e],
            }
            let (user, warn) = ito_theme::load_dir(&themes_dir(b));
            p.themes.extend(user);
            p.theme_warn = warn;
        }
        p.base = base;
        p
    }

    /// Look at the files and the OS's light or dark every 2 s from now on,
    /// and wake `ctx` when one has been read again.
    pub fn watch(&mut self, ctx: &egui::Context) {
        self.wake = Some(ctx.clone());
        let os_ctx = ctx.clone();
        self.os_light = ito_common::OsLight::watch(WATCH, move || os_ctx.request_repaint());
        let Some(base) = self.base.clone() else { return };
        let (common, themes) = (common_path(&base), themes_dir(&base));
        let mut paths = vec![common.clone(), themes.clone()];
        // A theme file edited in place; one added or taken away changes the
        // folder's time.
        if let Ok(entries) = std::fs::read_dir(&themes) {
            paths.extend(entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "toml")));
        }
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        ito_common::watch(paths, WATCH, move |changed| {
            let mut sent = true;
            if changed.contains(&common) {
                sent &= tx.send(Reload::Common(Common::read(&base))).is_ok();
            }
            if changed.iter().any(|p| p.starts_with(&themes)) {
                let (user, warn) = ito_theme::load_dir(&themes);
                let all = ito_theme::builtin().into_iter().chain(user).collect();
                sent &= tx.send(Reload::Themes(all, warn)).is_ok();
            }
            ctx.request_repaint();
            sent
        });
    }

    /// Fold in what the threads have read since the last frame.
    pub fn pump(&mut self) {
        while let Ok(r) = self.rx.try_recv() {
            match r {
                Reload::Common(Ok((mut c, w))) => {
                    if let Some(s) = self.scale_wanted {
                        if c.scale == Some(s) {
                            self.scale_wanted = None;
                        } else {
                            c.scale = Some(s);
                        }
                    }
                    (self.common, self.common_warn) = (c, w);
                }
                Reload::Common(Err(e)) => self.common_warn = vec![e],
                Reload::Themes(t, w) => (self.themes, self.theme_warn) = (t, w),
                Reload::Failed(e) => {
                    self.scale_wanted = None;
                    self.failed = Some(e);
                }
            }
        }
    }

    fn send_back(tx: &Sender<Reload>, wake: &Option<egui::Context>, r: Reload) {
        let _ = tx.send(r);
        if let Some(ctx) = wake {
            ctx.request_repaint();
        }
    }

    /// Write `changes` into common.toml on a thread, then read it again.
    pub fn common_change(&self, changes: Vec<CommonChange>, no_folder: &str) {
        if changes.is_empty() {
            return;
        }
        let (tx, wake) = (self.tx.clone(), self.wake.clone());
        let Some(base) = self.base.clone() else {
            Self::send_back(&tx, &wake, Reload::Failed(no_folder.into()));
            return;
        };
        std::thread::spawn(move || {
            let done = edit_common(&base, |text| changes.iter().try_fold(text.to_owned(), |t, c| c.apply(&t)));
            let r = match done {
                Ok(()) => Reload::Common(Common::read(&base)),
                Err(e) => Reload::Failed(e),
            };
            Self::send_back(&tx, &wake, r);
        });
    }

    /// The scale at once, and into common.toml on a thread of its own that
    /// writes only the newest of a burst of keys (kura's way).
    pub fn set_scale(&mut self, s: f32, no_folder: &str) {
        self.common.scale = Some(s);
        let Some(base) = self.base.clone() else {
            Self::send_back(&self.tx, &self.wake, Reload::Failed(no_folder.into()));
            return;
        };
        self.scale_wanted = Some(s);
        let tx = self.scale_tx.get_or_insert_with(|| {
            let (tx, rx) = channel::<f32>();
            let (back, wake) = (self.tx.clone(), self.wake.clone());
            std::thread::spawn(move || {
                while let Ok(mut s) = rx.recv() {
                    while let Ok(newer) = rx.try_recv() {
                        s = newer;
                    }
                    let r = match edit_common(&base, |t| ito_common::scale_change(s).apply(t)) {
                        Ok(()) => Reload::Common(Common::read(&base)),
                        Err(e) => Reload::Failed(e),
                    };
                    Self::send_back(&back, &wake, r);
                }
            });
            tx
        });
        let _ = tx.send(s);
    }

    /// Open common.toml (made empty first when there is none) with what the
    /// OS opens it with, on a thread.
    pub fn open_common(&self, no_folder: &str) {
        let (tx, wake) = (self.tx.clone(), self.wake.clone());
        let Some(base) = self.base.clone() else {
            Self::send_back(&tx, &wake, Reload::Failed(no_folder.into()));
            return;
        };
        std::thread::spawn(move || {
            let path = common_path(&base);
            let made = if path.exists() { Ok(()) } else { write_atomic(&path, "") };
            if let Err(e) = made.and_then(|()| open_path(&path)) {
                Self::send_back(&tx, &wake, Reload::Failed(e));
            }
        });
    }

    /// The theme common.toml picks, in force from this frame on. Called
    /// every frame (the OS may have gone light or dark).
    pub fn apply_theme(&mut self, ctx: &egui::Context) {
        let os_light = self.os_light.light_or(ctx.system_theme() == Some(egui::Theme::Light));
        let (choice, dark, light) = self.common.theme_choice(None);
        let colors = match ito_theme::pick(&self.themes, &choice, &dark, &light, os_light) {
            Ok(t) => {
                self.theme_err = None;
                t.colors
            }
            Err(e) => {
                self.theme_err = Some(e);
                self.themes[0].colors
            }
        };
        if colors != ito_theme::colors() {
            ito_theme::set(colors);
            ctx.set_visuals(colors.visuals());
        }
    }
}

#[cfg(windows)]
fn opener() -> std::process::Command {
    std::process::Command::new("explorer")
}

#[cfg(target_os = "macos")]
fn opener() -> std::process::Command {
    std::process::Command::new("open")
}

#[cfg(not(any(windows, target_os = "macos")))]
fn opener() -> std::process::Command {
    std::process::Command::new("xdg-open")
}

fn open_path(path: &Path) -> Result<(), String> {
    // explorer answers 1 even when it opened the file, so only a failure to
    // start it is said.
    opener().arg(path).status().map(drop).map_err(|e| format!("{}: {e}", path.display()))
}
