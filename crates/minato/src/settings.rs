//! The settings screen (`,`): `ito-prefs` draws the nav and the cards, and
//! the rows common to every uchmk app (the language, the size, the clock,
//! the theme) write common.toml. minato's own are on the Views page (whose
//! repositories, the inbox rules, the saved searches, the writing box's
//! fields), the Keys page (each operation's keys, written to keymap.toml) and
//! the Notifications page (how a watched run's end is told).

use egui::{vec2, Align2, FontId, Sense, Stroke, StrokeKind, Ui};
use ito_common::{themes_dir, CommonChange};
use ito_prefs::{section, Look, MoreWords, Words};
use minato_core::prefs::{is_http, Notify, FORMATS, SOUNDS};

use crate::app::{App, Hook, Ring};

const GENERAL: usize = 0;
const THEME: usize = 1;
const VIEWS: usize = 2;
const KEYS: usize = 3;
const NOTIFY: usize = 4;
const ADVANCED: usize = 5;
const ABOUT: usize = 6;

/// The keymap's layers and the words that name them on the Keys page.
const LAYERS: [(&str, &str); 8] = [
    ("main", "keys.main"),
    ("inbox", "view.inbox"),
    ("list", "view.list"),
    ("actions", "view.actions"),
    ("board", "view.board"),
    ("status", "view.status"),
    ("security", "view.security"),
    ("notes", "keys.notes"),
];

/// The words of the Notifications page, for the search.
const NOTIFY_WORDS: [&str; 13] = [
    "settings.notify_when",
    "settings.notify_system",
    "settings.notify_sound",
    "settings.notify_webhook",
    "settings.webhook_add",
    "settings.webhook_url",
    "settings.webhook_format",
    "settings.sound_ok",
    "settings.sound_bad",
    "settings.sound_idle",
    "settings.focus",
    "settings.notify_test",
    "settings.notify_reset",
];

/// The rows on each page, for the search.
fn index(w: &'static Words, more: &'static MoreWords, lang: &str) -> Vec<(usize, &'static str)> {
    let notify = NOTIFY_WORDS.iter().map(|k| (NOTIFY, crate::i18n::tr(lang, k)));
    let keys = ito_prefs::keys_index(ito_prefs::KeyWords::of(lang)).into_iter().map(|t| (KEYS, t));
    let layers = LAYERS.iter().map(|(_, k)| (KEYS, crate::i18n::tr(lang, k)));
    vec![
        (GENERAL, w.language),
        (GENERAL, w.scale),
        (GENERAL, w.clock),
        (GENERAL, w.show_time),
        (GENERAL, w.time_format),
        (GENERAL, w.show_date),
        (GENERAL, w.date_format),
        (GENERAL, w.weekday),
        (GENERAL, "language"),
        (GENERAL, "scale"),
        (GENERAL, "[clock]"),
        (GENERAL, "[window]"),
        (THEME, w.mode),
        (THEME, w.themes),
        (THEME, w.preview),
        (THEME, "theme"),
        (THEME, "dark_theme"),
        (THEME, "light_theme"),
        (KEYS, "keymap.toml"),
        (KEYS, "prepend_keymap"),
    ]
    .into_iter()
    .chain(notify)
    .chain(keys)
    .chain(layers)
    .chain(ito_prefs::window_index(more).into_iter().chain(ito_prefs::startup_index(more)).map(|t| (GENERAL, t)))
    .chain(ito_prefs::advanced_index(more).into_iter().chain(ito_prefs::backup_index(more)).map(|t| (ADVANCED, t)))
    .chain(ito_prefs::about_index(more, w).into_iter().map(|t| (ABOUT, t)))
    .collect()
}

pub fn show(ui: &mut Ui, app: &mut App) {
    let w = Words::of(&app.lang);
    let more = MoreWords::of(&app.lang);
    poll_startup(ui, app);
    // While a key is being set the page takes every key, Esc included.
    let keys = if app.prefs.keys_edit.busy() { ito_prefs::Keys::default() } else { ito_prefs::Keys::standard(ui, false) };
    let pages = [
        (app.tr("settings.general"), app.tr("settings.general_lead")),
        (app.tr("settings.theme"), app.tr("settings.theme_lead")),
        (app.tr("settings.views"), app.tr("settings.views_lead")),
        (app.tr("settings.keys"), app.tr("settings.keys_lead")),
        (app.tr("settings.notify"), app.tr("settings.notify_lead")),
        ito_prefs::Page::Advanced.words(more),
        ito_prefs::Page::About.words(more),
    ];
    let index = index(w, more, &app.lang);
    let nav = ito_prefs::Nav { pages: &pages, index: &index, words: w, file: "common.toml" };
    let mut asked: Vec<CommonChange> = Vec::new();
    let lang = app.lang.clone();
    let tr = |k: &'static str| crate::i18n::tr(&lang, k);
    let me = app.viewer.clone().unwrap_or_default();
    let layers = app.keys.layers().to_vec();
    let kw = ito_prefs::KeyWords::of(&lang);
    let own =
        app.prefs.base.as_deref().map_or_else(|| crate::keymap::FILE.to_owned(), |b| crate::prefs::keymap_path(b).display().to_string());
    let mut key_edits = Vec::new();
    let prefs = &mut app.prefs;
    let keys_edit = &mut prefs.keys_edit;
    let before = app.settings.clone();
    let st = &mut app.settings;
    let scope_text = &mut app.scope_text;
    let searches_text = &mut app.searches_text;
    let drafts = &mut app.fields_typed;
    let mut searches_changed = false;
    let mut bug = false;
    let mut forget = false;
    let mut startup_toggled = None;
    let mut advanced = Vec::new();
    let mut backups = Vec::new();
    let startup_now = app.startup;
    let state_dir = ito_common::state_dir("minato");
    let mut test = None;
    let (common, themes, base) = (&prefs.common, &prefs.themes, prefs.base.as_deref());
    let out = ito_prefs::show(ui, ito_theme::colors(), &mut prefs.state, &nav, keys, |ui, page, l| match page {
        GENERAL => {
            section(ui, l, tr("settings.display"), |ui| {
                let now = common.language.as_deref().unwrap_or("auto");
                asked.extend(ito_prefs::language_row(ui, l, w, now, ""));
                ito_prefs::sep(ui, l);
                asked.extend(ito_prefs::scale_row(ui, l, w, common.scale_or_one(), ""));
            });
            asked.extend(ito_prefs::clock_card(ui, l, w, &common.clock_or(None)));
            let out = ito_prefs::window_card(ui, l, more, &common.window_or_default());
            asked.extend(out.changes);
            forget |= out.forget;
            startup_toggled = ito_prefs::startup_card(ui, l, more, startup_now);
        }
        ADVANCED => {
            advanced.extend(ito_prefs::advanced_card(ui, l, more, state_dir.is_some()));
            let file = ito_common::home_dir().unwrap_or_default().join("minato-settings.toml");
            backups.extend(ito_prefs::backup_card(ui, l, more, &file));
        }
        ABOUT => {
            let system = ito_bugreport::os_line();
            let license = Some(env!("CARGO_PKG_LICENSE")).filter(|x| !x.is_empty());
            let about =
                ito_prefs::About { version: env!("CARGO_PKG_VERSION"), commit: option_env!("MINATO_COMMIT"), system: &system, license };
            bug |= !ito_prefs::about_page(ui, l, more, w, "minato", &about).is_empty();
        }
        THEME => {
            let (t, d, li) = common.theme_choice(None);
            let note = match base {
                Some(b) => format!("{} {}", tr("settings.theme_note"), themes_dir(b).display()),
                None => tr("settings.no_folder").into(),
            };
            asked.extend(ito_prefs::theme_page(ui, l, w, themes, (&t, &d, &li), &note, |ui| {
                let tabs = ["view.inbox", "view.list", "view.actions", "view.board"].map(tr);
                preview(ui, tr("settings.sample"), tabs, ["run.ok", "run.bad", "run.running"].map(tr))
            }));
        }
        NOTIFY => test = notify_page(ui, l, &tr, &mut st.notify, drafts),
        KEYS => {
            let layer_name = |n: &str| LAYERS.iter().find(|(id, _)| *id == n).map_or_else(|| n.to_owned(), |(_, k)| tr(k).to_owned());
            let desc = |d: &str| crate::i18n::desc(&lang, d);
            // Keys minato reads before the keymap: bound, they never run.
            let extra = |_: &str, on: &[ito_keys::Key]| -> Vec<String> {
                crate::keymap::Keys::fixed(on).map(|k| tr("keys.fixed").replace("{}", &k.human())).into_iter().collect()
            };
            let table = ito_prefs::KeyTable {
                now: &layers,
                built_in: crate::keymap::Keys::built_in_layers(),
                noop: crate::keymap::NOOP,
                own: &own,
                layer_name: &layer_name,
                tr: &desc,
                extra: &extra,
                unbound: &[],
                sup: false,
            };
            key_edits.extend(ito_prefs::keys_page(ui, l, kw, &table, keys_edit).unwrap_or_default());
        }
        VIEWS => {
            section(ui, l, tr("settings.scope"), |ui| {
                let rows = [("settings.owner", "settings.owner_note", me.as_str()), ("settings.project", "settings.project_note", "")];
                for (k, (label, note, hint)) in rows.into_iter().enumerate() {
                    if k > 0 {
                        ito_prefs::sep(ui, l);
                    }
                    // Taken on Enter (or on leaving the box), not on each key:
                    // each change asks GitHub again.
                    let done = ito_prefs::row(ui, l, tr(label), tr(note), |ui| {
                        let edit = egui::TextEdit::singleline(&mut scope_text[k])
                            .hint_text(hint)
                            .desired_width(200.0)
                            .font(FontId::monospace(12.5));
                        ui.add(edit).lost_focus()
                    });
                    if done && k == 0 {
                        st.set_owner(&scope_text[0]);
                        scope_text[0] = st.owners_line();
                    } else if done {
                        st.project = scope_text[1].trim().to_string();
                    }
                }
            });
            section(ui, l, tr("settings.rules"), |ui| {
                let mut shift = None;
                for at in 0..st.rules.1.len() {
                    let r = minato_core::inbox::RULES[st.rules.1[at]];
                    let on = st.rules.0[r.index()];
                    let flipped = ito_prefs::row(ui, l, tr(r.why()), tr("settings.rule_note"), |ui| {
                        let switched = ito_prefs::switch(ui, l, on);
                        if ui.small_button("▼").on_hover_text(tr("settings.move_down")).clicked() {
                            shift = Some((at, 1));
                        }
                        if ui.small_button("▲").on_hover_text(tr("settings.move_up")).clicked() {
                            shift = Some((at, -1));
                        }
                        switched
                    });
                    if flipped {
                        st.rules.0[r.index()] = !on;
                    }
                }
                if let Some((at, step)) = shift {
                    st.rules.shift(at, step);
                }
            });
            section(ui, l, tr("settings.auto_done"), |ui| {
                if ito_prefs::row(ui, l, tr("settings.auto_done_row"), tr("settings.auto_done_note"), |ui| {
                    ito_prefs::switch(ui, l, st.auto_done)
                }) {
                    st.auto_done = !st.auto_done;
                }
            });
            section(ui, l, tr("settings.searches"), |ui| {
                ui.label(egui::RichText::new(tr("settings.searches_note")).color(ito_theme::colors().dim).size(12.0));
                let edit =
                    egui::TextEdit::multiline(searches_text).desired_rows(4).desired_width(f32::INFINITY).font(FontId::monospace(12.5));
                searches_changed |= ui.add(edit).changed();
            });
            section(ui, l, tr("settings.fields"), |ui| {
                let names = ["pick.labels", "pick.assignees", "pick.status", "pick.milestone", "pick.priority", "pick.reviewers"];
                let mut shift = None;
                for at in 0..names.len() {
                    let k = st.order[at];
                    let on = st.fields[k];
                    let flipped = ito_prefs::row(ui, l, tr(names[k]), tr("settings.field_note"), |ui| {
                        let switched = ito_prefs::switch(ui, l, on);
                        if ui.small_button("▼").on_hover_text(tr("settings.move_down")).clicked() {
                            shift = Some((at, 1));
                        }
                        if ui.small_button("▲").on_hover_text(tr("settings.move_up")).clicked() {
                            shift = Some((at, -1));
                        }
                        switched
                    });
                    if flipped {
                        st.fields[k] = !on;
                    }
                }
                if let Some((at, step)) = shift {
                    st.shift(at, step);
                }
            });
        }
        _ => {}
    });
    if let Some(test) = test {
        let n = &app.settings.notify;
        let (title, body) = (app.tr("notify.test_title").to_string(), app.tr("notify.test_body").to_string());
        let hook = |h: &minato_core::prefs::Hook| Hook {
            url: h.url.trim().into(),
            format: h.format.clone(),
            level: ito_notify::webhook::Level::Done,
            folder: String::new(),
        };
        let hooks = n
            .hooks
            .iter()
            .enumerate()
            .filter(|(k, h)| is_http(&h.url) && matches!(test, Test::All) || test == Test::Hook(*k) && is_http(&h.url))
            .map(|(_, h)| hook(h))
            .collect();
        // A webhook's own button tries only that one: no window notification, no sound.
        let all = test == Test::All;
        app.rings.push(Ring { title, body, system: all, sound: all.then(|| n.sounds[0].clone()), focus: n.focus, hooks });
        crate::net::ring(app);
    }
    if bug {
        app.bug_report();
    }
    if forget {
        if let Some(dir) = &state_dir {
            let dir = dir.clone();
            std::thread::spawn(move || {
                let _ = ito_prefs::forget_place(&dir);
            });
        }
    }
    if let Some(on) = startup_toggled {
        set_startup(ui.ctx(), app, on);
    }
    for b in backups {
        app.prefs.backup(b, [app.tr("backup.exported").to_owned(), app.tr("backup.imported").to_owned()]);
    }
    for a in advanced {
        let no_folder = crate::i18n::tr(&app.lang, "settings.no_folder");
        match a {
            ito_prefs::Advanced::OpenConfigDir => match app.prefs.base.clone() {
                Some(b) => app.prefs.open_dir(b),
                None => app.say(no_folder),
            },
            ito_prefs::Advanced::OpenStateDir => {
                if let Some(d) = state_dir.clone() {
                    app.prefs.open_dir(d);
                }
            }
            ito_prefs::Advanced::Reset => {
                // config.toml is moved aside on a thread, so nothing is written over it here.
                app.prefs.reset(no_folder);
                app.settings = Default::default();
                app.scope_text = [app.settings.owners_line(), app.settings.project.clone()];
                app.searches_text.clear();
                app.sync_searches();
            }
        }
    }
    if searches_changed {
        app.sync_searches();
        app.save_searches();
    }
    if app.settings != before {
        app.save_settings();
    }
    if out.close {
        app.escape();
    }
    if out.open_file {
        let no_folder = crate::i18n::tr(&app.lang, "settings.no_folder");
        app.prefs.open_common(no_folder);
    }
    // Gone to another page: the keys stop being taken for this one.
    if app.prefs.state.page != KEYS {
        app.prefs.keys_edit.capture = None;
        app.prefs.keys_edit.listening = false;
    }
    let no_folder = crate::i18n::tr(&app.lang, "settings.no_folder");
    app.prefs.keymap_edit(key_edits, no_folder);
    app.prefs.common_change(asked, no_folder);
}

/// Take the OS's answer about starting at sign-in, and ask when nothing is
/// known yet (once the General page is up). The registry and the autostart
/// folders are read on a thread.
fn poll_startup(ui: &Ui, app: &mut App) {
    if let Some(v) = app.startup_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
        app.startup = Some(v);
        app.startup_rx = None;
    }
    if app.startup.is_none() && app.startup_rx.is_none() && app.prefs.state.page == GENERAL {
        app.startup_rx = Some(startup_task(ui.ctx(), None));
    }
}

/// The switch was clicked: set it on a thread, then read what the OS has.
fn set_startup(ctx: &egui::Context, app: &mut App, on: bool) {
    app.startup = None;
    app.startup_rx = Some(startup_task(ctx, Some(on)));
}

/// Read (and first set, when `set` is given) the sign-in start on a thread.
fn startup_task(ctx: &egui::Context, set: Option<bool>) -> std::sync::mpsc::Receiver<bool> {
    let (tx, rx) = std::sync::mpsc::channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        if let (Some(on), Ok(exe)) = (set, std::env::current_exe()) {
            let _ = ito_common::startup::set("minato", &exe, &[], on);
        }
        let _ = tx.send(ito_common::startup::enabled("minato"));
        ctx.request_repaint();
    });
    rx
}

/// What the "Send a test" buttons asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Test {
    /// The button at the top: every way, every webhook.
    All,
    /// A webhook's own button, by its place in the list.
    Hook(usize),
}

/// The Notifications page: a switch per way of telling and per end (passed,
/// failed, cancelled), then the sounds and the webhooks. What test was asked
/// for.
fn notify_page(
    ui: &mut Ui,
    l: Look,
    tr: &dyn Fn(&'static str) -> &'static str,
    n: &mut Notify,
    drafts: &mut ito_prefs::Drafts,
) -> Option<Test> {
    let c = l.c;
    let mut test = None;
    ui.horizontal(|ui| {
        if ito_prefs::button(ui, l, tr("settings.notify_test")) {
            test = Some(Test::All);
        }
        if ito_prefs::button(ui, l, tr("settings.notify_reset")) {
            // The webhooks stay: they are not a choice but places.
            *n = Notify { hooks: std::mem::take(&mut n.hooks), ..Notify::default() };
        }
    });
    ui.add_space(8.0);
    section(ui, l, tr("settings.notify_when"), |ui| {
        // tsumugi's table: the way to tell, then a column per end, the
        // label's column 1.6 times as wide as each end's.
        let ends = [("run.ok", c.done), ("run.bad", c.err), ("run.idle", c.dim)];
        let gap = 16.0;
        let width = ui.available_width();
        let unit = (width - 3.0 * gap) / 4.6;
        let columns = |row: egui::Rect| -> [egui::Rect; 4] {
            let mut x = row.left();
            let mut take = |w: f32| {
                let r = egui::Rect::from_min_max(egui::pos2(x, row.top()), egui::pos2(x + w, row.bottom()));
                x += w + gap;
                r
            };
            [take(unit * 1.6), take(unit), take(unit), take(unit)]
        };
        let (head, _) = ui.allocate_exact_size(vec2(width, 40.0), Sense::hover());
        for ((key, color), r) in ends.iter().zip(&columns(head)[1..]) {
            let text = ui.fonts_mut(|f| f.layout_no_wrap(tr(key).to_string(), FontId::proportional(13.0), *color));
            let left = r.center().x - (text.size().x + 14.0) / 2.0;
            ui.painter().circle_filled(egui::pos2(left + 4.0, r.center().y), 4.0, *color);
            ui.painter().galley(egui::pos2(left + 14.0, r.center().y - text.size().y / 2.0), text, *color);
        }
        let rows = [
            ("settings.notify_system", "settings.notify_system_note", &mut n.system),
            ("settings.notify_sound", "settings.notify_sound_note", &mut n.sound),
        ];
        for (label, note, list) in rows {
            ito_prefs::sep(ui, l);
            let (row, _) = ui.allocate_exact_size(vec2(width, 66.0), Sense::hover());
            let cols = columns(row);
            let words = egui::Rect::from_center_size(cols[0].center(), vec2(cols[0].width(), 40.0));
            // Children of their own: a scope would move the card's cursor
            // to their bottom and take back the row's room.
            let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(words));
            inner.spacing_mut().item_spacing.y = 2.0;
            inner.label(egui::RichText::new(tr(label)).size(13.0).color(c.strong()));
            inner.add(egui::Label::new(egui::RichText::new(tr(note)).size(12.0).color(c.dim)).truncate());
            for (on, r) in list.iter_mut().zip(&cols[1..]) {
                let at = egui::Rect::from_center_size(r.center(), vec2(40.0, 22.0));
                if ito_prefs::switch(&mut ui.new_child(egui::UiBuilder::new().max_rect(at)), l, *on) {
                    *on = !*on;
                }
            }
        }
    });
    section(ui, l, tr("settings.notify_sounds"), |ui| {
        let names: Vec<(&str, &str)> = SOUNDS.iter().map(|s| (*s, tr(sound_word(s)))).collect();
        for (k, label) in ["settings.sound_ok", "settings.sound_bad", "settings.sound_idle"].into_iter().enumerate() {
            if k > 0 {
                ito_prefs::sep(ui, l);
            }
            let picked = ito_prefs::row(ui, l, tr(label), "", |ui| {
                if ui.small_button("▶").on_hover_text(tr("settings.sound_play")).clicked() {
                    ito_notify::sound::play(&n.sounds[k]);
                }
                ito_prefs::select(ui, label, n.sounds[k].as_str(), &names).map(str::to_string)
            });
            if let Some(s) = picked {
                n.sounds[k] = s;
            }
        }
        ito_prefs::sep(ui, l);
        if ito_prefs::row(ui, l, tr("settings.focus"), tr("settings.focus_note"), |ui| ito_prefs::switch(ui, l, n.focus)) {
            n.focus = !n.focus;
        }
    });
    section(ui, l, tr("settings.webhook"), |ui| {
        let count = n.hooks.len();
        let names: Vec<(&str, &str)> = FORMATS.iter().map(|f| (*f, format_word(f))).collect();
        let mut gone = None;
        for (k, h) in n.hooks.iter_mut().enumerate() {
            if k > 0 {
                ito_prefs::sep(ui, l);
            }
            let label =
                if count == 1 { tr("settings.notify_webhook").to_string() } else { format!("{} {}", tr("settings.notify_webhook"), k + 1) };
            let note = if h.url.is_empty() { tr("settings.webhook_empty") } else { "" };
            ito_prefs::row(ui, l, &label, note, |ui| {
                // Right to left: the first one added is the rightmost.
                if ito_prefs::button(ui, l, tr("settings.webhook_remove")) {
                    gone = Some(k);
                }
                if ito_prefs::button(ui, l, tr("settings.webhook_test")) {
                    test = Some(Test::Hook(k));
                }
            });
            ito_prefs::sep(ui, l);
            let typed = ito_prefs::row(ui, l, tr("settings.webhook_url"), tr("settings.webhook_url_note"), |ui| {
                ito_prefs::field(ui, drafts, &format!("webhook-{k}"), &h.url, "https://ntfy.sh/my-topic", 260.0)
            });
            if let Some(t) = typed {
                h.url = t.trim().to_string();
            }
            if !h.url.is_empty() && !is_http(&h.url) {
                ito_prefs::status(ui, tr("settings.webhook_bad"), c.err);
                ui.add_space(4.0);
            }
            ito_prefs::sep(ui, l);
            let picked = ito_prefs::row(ui, l, tr("settings.webhook_format"), tr("settings.webhook_format_note"), |ui| {
                ito_prefs::select(ui, &format!("webhook-format-{k}"), h.format.as_str(), &names)
            });
            if let Some(f) = picked {
                h.format = f.to_string();
            }
            ito_prefs::sep(ui, l);
            ito_prefs::row(ui, l, tr("settings.webhook_ends"), tr("settings.webhook_ends_note"), |ui| {
                // Right to left, so the last end is placed first.
                for (end, on) in ["run.idle", "run.bad", "run.ok"].into_iter().zip(h.on.iter_mut().rev()) {
                    if ito_prefs::switch(ui, l, *on) {
                        *on = !*on;
                    }
                    ui.label(egui::RichText::new(tr(end)).size(12.0).color(c.dim));
                }
            });
        }
        if let Some(k) = gone {
            n.hooks.remove(k);
        }
        if count > 0 {
            ito_prefs::sep(ui, l);
        }
        let note = if count == 0 { tr("settings.webhook_none") } else { "" };
        if ito_prefs::row(ui, l, tr("settings.notify_webhook"), note, |ui| ito_prefs::button(ui, l, tr("settings.webhook_add"))) {
            n.hooks.push(minato_core::prefs::Hook::new());
        }
    });
    test
}

/// A webhook's form as it is written.
fn format_word(f: &str) -> &'static str {
    match f {
        "slack" => "Slack",
        "discord" => "Discord",
        "json" => "JSON",
        _ => "ntfy",
    }
}

/// The key of a sound's name.
fn sound_word(s: &str) -> &'static str {
    match s {
        "chime" => "settings.chime",
        "low" => "settings.low",
        "alert" => "settings.alert",
        _ => "settings.default",
    }
}

/// minato in the theme in force: the title bar's tabs and a row of the
/// keybar, small, in the language in force. A name that does not fit is
/// left out, not cut.
fn preview(ui: &mut Ui, sample: &str, tabs: [&str; 4], runs: [&str; 3]) {
    let c = ito_theme::colors();
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width().min(420.0), 96.0), Sense::hover());
    let p = ui.painter_at(r);
    p.rect_filled(r, 8.0, c.bg);
    p.rect_stroke(r, 8.0, Stroke::new(1.0, c.border), StrokeKind::Inside);
    let inner = r.shrink(12.0);
    let mono = FontId::monospace(12.0);
    p.text(inner.left_top(), Align2::LEFT_TOP, sample, FontId::proportional(12.0), c.dim);
    let tabs = tabs.into_iter().enumerate().map(|(k, name)| (format!("{} {name}", k + 1), if k == 0 { crate::ui::ACCENT } else { c.fg }));
    let runs = ["●", "✗", "▶"].into_iter().zip(runs).map(|(m, name)| (format!("{m} {name}"), c.fg));
    for (row, items) in [(26.0, tabs.collect::<Vec<_>>()), (52.0, runs.collect())] {
        let mut x = inner.left();
        for (text, color) in items {
            let g = p.layout_no_wrap(text, mono.clone(), color);
            if x + g.size().x > inner.right() {
                break;
            }
            let w = g.size().x;
            p.galley(egui::pos2(x, inner.top() + row), g, color);
            x += w + 14.0;
        }
    }
    p.circle_filled(egui::pos2(inner.right() - 8.0, inner.top() + 8.0), 4.0, c.done);
}

#[cfg(test)]
mod tests {
    use ito_prefs::Page;

    #[test]
    fn pages_follow_ito_default_order() {
        let ours = [GENERAL, THEME, VIEWS, KEYS, NOTIFY, ADVANCED, ABOUT];
        assert!(ours.windows(2).all(|w| w[0] < w[1]));
        let has = [Page::Theme, Page::App, Page::Keys, Page::Notifications, Page::Advanced];
        assert_eq!(
            ito_prefs::default_order(&has),
            [Page::General, Page::Theme, Page::App, Page::Keys, Page::Notifications, Page::Advanced, Page::About]
        );
    }

    use super::*;
    use egui::{pos2, Rect};

    /// The Keys page takes the keys pressed while a key is being set (Esc
    /// and Enter included, which would close the screen), and writes only
    /// the key set to keymap.toml once Enter sets it.
    #[test]
    fn the_keys_page_takes_a_key_and_writes_it_on_enter() {
        let dir = std::env::temp_dir().join(format!("minato-settings-keys-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut a = App::new(crate::prefs::Prefs::empty());
        a.prefs.base = Some(dir.clone());
        a.prefs.state.page = KEYS;
        let ctx = egui::Context::default();
        let mut time = 0.0;
        let mut frame = |a: &mut App, events: Vec<egui::Event>| {
            time += 0.1;
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 800.0))),
                time: Some(time),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| show(ui, a));
            });
            out.textures_delta.clear();
        };
        let key = |key, modifiers, pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
        let mut press = |a: &mut App, k, m| {
            frame(a, vec![key(k, m, true)]);
            frame(a, vec![key(k, m, false)]);
        };
        let start = |a: &mut App| {
            a.prefs.keys_edit.capture =
                Some(ito_prefs::Capture { layer: "main".into(), run: "notes".into(), keys: Vec::new(), replace: None });
        };
        let file = crate::prefs::keymap_path(&dir);
        // Esc gives up the key, and the screen stays open.
        start(&mut a);
        a.mode = crate::app::Mode::Settings;
        press(&mut a, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(a.prefs.keys_edit.capture.is_none(), "Esc gave the key up");
        assert!(a.mode == crate::app::Mode::Settings, "the screen stays open");
        // Ctrl+Shift+Enter is a key; nothing is written before Enter.
        start(&mut a);
        let cs = egui::Modifiers { ctrl: true, shift: true, command: true, ..Default::default() };
        press(&mut a, egui::Key::Enter, cs);
        assert!(!file.exists(), "a key pressed is not written yet");
        assert_eq!(a.prefs.keys_edit.capture.as_ref().map(|c| c.keys.len()), Some(1), "{:?}", a.prefs.keys_edit.capture);
        press(&mut a, egui::Key::Enter, egui::Modifiers::NONE);
        assert!(a.mode == crate::app::Mode::Settings, "Enter set the key and did not close the screen");
        let mut text = String::new();
        for _ in 0..500 {
            text = std::fs::read_to_string(&file).unwrap_or_default();
            if text.contains("notes") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(text.contains("notes"), "keymap.toml was not written within 5 s");
        assert!(text.contains("<C-S-Enter>") && text.contains(r#"run = "notes""#) && text.contains("prepend_keymap"), "{text}");
        assert!(!text.contains("none") && !text.contains("down"), "only the key set is written: {text}");
        // Taking keys, then gone to another page: the keys stop being taken.
        start(&mut a);
        a.prefs.keys_edit.listening = true;
        a.prefs.state.page = NOTIFY;
        press(&mut a, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(!a.prefs.keys_edit.busy(), "another page takes no keys for the Keys page");
        assert!(a.mode == crate::app::Mode::Settings, "the Esc that gave the key up did not close the screen");
        // Nothing taken: Esc closes the screen again.
        press(&mut a, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(a.mode != crate::app::Mode::Settings, "Esc with nothing taken closes the settings");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The Keys page's words are found by the search.
    #[test]
    fn the_search_finds_the_keys_page() {
        let w = Words::of("en");
        let idx = index(w, MoreWords::of("en"), "en");
        assert!(idx.iter().any(|(p, t)| *p == KEYS && *t == "keymap.toml"));
        assert!(idx.iter().any(|(p, t)| *p == KEYS && *t == "Everywhere"));
    }
}
