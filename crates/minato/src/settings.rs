//! The settings screen (`,`): `ito-prefs` draws the nav and the cards, and
//! the rows common to every uchmk app (the language, the size, the clock,
//! the theme) write common.toml. minato's own (whose repositories, the
//! inbox rules, the saved searches, the writing box's fields) are on its page.

use egui::{vec2, Align2, FontId, Sense, Stroke, StrokeKind, Ui};
use ito_common::{themes_dir, CommonChange};
use ito_prefs::{section, Words};

use crate::app::App;

const GENERAL: usize = 0;
const THEME: usize = 1;
const MINATO: usize = 2;

/// The rows on each page, for the search.
fn index(w: &'static Words) -> Vec<(usize, &'static str)> {
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
}

pub fn show(ui: &mut Ui, app: &mut App) {
    let w = Words::of(&app.lang);
    let keys = ito_prefs::Keys::standard(ui, false);
    let pages = [
        (app.tr("settings.general"), app.tr("settings.general_lead")),
        (app.tr("settings.theme"), app.tr("settings.theme_lead")),
        (app.tr("settings.minato"), app.tr("settings.minato_lead")),
    ];
    let index = index(w);
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
    let mut searches_changed = false;
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
        MINATO => {
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
                        st.owner = minato_core::prefs::login(&scope_text[0]);
                        scope_text[0] = st.owner.clone();
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
