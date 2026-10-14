//! The settings screen (`,`): `ito-prefs` draws the nav and the cards, and
//! the rows common to every uchmk app (the language, the size, the clock,
//! the theme) write common.toml. minato's own are on the Notifications
//! page (how a watched run's end is told) and the Views page (whose
//! repositories, the inbox rules, the saved searches, the writing box's
//! fields).

use egui::{vec2, Align2, FontId, Sense, Stroke, StrokeKind, Ui};
use ito_common::{themes_dir, CommonChange};
use ito_prefs::{section, Look, Words};
use minato_core::prefs::{is_http, Notify, FORMATS, SOUNDS};

use crate::app::{App, Hook, Ring};

const GENERAL: usize = 0;
const THEME: usize = 1;
const NOTIFY: usize = 2;
const VIEWS: usize = 3;

/// The words of the Notifications page, for the search.
const NOTIFY_WORDS: [&str; 12] = [
    "settings.notify_when",
    "settings.notify_system",
    "settings.notify_sound",
    "settings.notify_webhook",
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
fn index(w: &'static Words, lang: &str) -> Vec<(usize, &'static str)> {
    let notify = NOTIFY_WORDS.iter().map(|k| (NOTIFY, crate::i18n::tr(lang, k)));
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
        (THEME, w.mode),
        (THEME, w.themes),
        (THEME, w.preview),
        (THEME, "theme"),
        (THEME, "dark_theme"),
        (THEME, "light_theme"),
    ]
    .into_iter()
    .chain(notify)
    .collect()
}

pub fn show(ui: &mut Ui, app: &mut App) {
    let w = Words::of(&app.lang);
    let keys = ito_prefs::Keys::standard(ui, false);
    let pages = [
        (app.tr("settings.general"), app.tr("settings.general_lead")),
        (app.tr("settings.theme"), app.tr("settings.theme_lead")),
        (app.tr("settings.notify"), app.tr("settings.notify_lead")),
        (app.tr("settings.views"), app.tr("settings.views_lead")),
    ];
    let index = index(w, &app.lang);
    let nav = ito_prefs::Nav { pages: &pages, index: &index, words: w, file: "common.toml" };
    let mut asked: Vec<CommonChange> = Vec::new();
    let lang = app.lang.clone();
    let tr = |k: &'static str| crate::i18n::tr(&lang, k);
    let me = app.viewer.clone().unwrap_or_default();
    let prefs = &mut app.prefs;
    let before = app.settings.clone();
    let st = &mut app.settings;
    let scope_text = &mut app.scope_text;
    let searches_text = &mut app.searches_text;
    let drafts = &mut app.fields_typed;
    let mut searches_changed = false;
    let mut test = false;
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
    if test {
        let n = &app.settings.notify;
        let (title, body) = (app.tr("notify.test_title").to_string(), app.tr("notify.test_body").to_string());
        let hook = is_http(&n.webhook_url).then(|| Hook {
            url: n.webhook_url.trim().into(),
            format: n.webhook_format.clone(),
            level: ito_notify::webhook::Level::Done,
            folder: String::new(),
        });
        app.rings.push(Ring { title, body, system: true, sound: Some(n.sounds[0].clone()), focus: n.focus, hook });
        crate::net::ring(app);
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
    let no_folder = crate::i18n::tr(&app.lang, "settings.no_folder");
    app.prefs.common_change(asked, no_folder);
}

/// The Notifications page: a switch per way of telling and per end (passed,
/// failed, cancelled), then the sounds and the webhook. Whether the test was
/// asked for.
fn notify_page(ui: &mut Ui, l: Look, tr: &dyn Fn(&'static str) -> &'static str, n: &mut Notify, drafts: &mut ito_prefs::Drafts) -> bool {
    let c = l.c;
    let mut test = false;
    ui.horizontal(|ui| {
        test = ito_prefs::button(ui, l, tr("settings.notify_test"));
        if ito_prefs::button(ui, l, tr("settings.notify_reset")) {
            // The webhook's address and format stay: they are not a choice but a place.
            *n = Notify {
                webhook_url: std::mem::take(&mut n.webhook_url),
                webhook_format: std::mem::take(&mut n.webhook_format),
                ..Notify::default()
            };
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
            ("settings.notify_webhook", "settings.notify_webhook_note", &mut n.webhook),
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
        let typed = ito_prefs::row(ui, l, tr("settings.webhook_url"), tr("settings.webhook_url_note"), |ui| {
            ito_prefs::field(ui, drafts, "webhook", &n.webhook_url, "https://ntfy.sh/my-topic", 260.0)
        });
        if let Some(t) = typed {
            n.webhook_url = t.trim().to_string();
        }
        if !n.webhook_url.is_empty() && !is_http(&n.webhook_url) {
            ito_prefs::status(ui, tr("settings.webhook_bad"), c.err);
            ui.add_space(4.0);
        }
        ito_prefs::sep(ui, l);
        let names: Vec<(&str, &str)> = FORMATS.iter().map(|f| (*f, format_word(f))).collect();
        let picked = ito_prefs::row(ui, l, tr("settings.webhook_format"), tr("settings.webhook_format_note"), |ui| {
            ito_prefs::select(ui, "webhook-format", n.webhook_format.as_str(), &names)
        });
        if let Some(f) = picked {
            n.webhook_format = f.to_string();
        }
    });
    test
}

/// A webhook's form as it is written.
fn format_word(f: &str) -> &'static str {
    match f {
        "slack" => "Slack",
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
