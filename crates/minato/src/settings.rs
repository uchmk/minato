//! The settings screen (`,`): `ito-prefs` draws the nav and the cards, and
//! the rows common to every uchmk app (the language, the size, the clock,
//! the theme) write common.toml. minato has no settings of its own yet.

use egui::{Align2, FontId, Sense, Stroke, StrokeKind, Ui, vec2};
use ito_common::{CommonChange, themes_dir};
use ito_prefs::{Words, section};

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
    let prefs = &mut app.prefs;
    let before = app.settings;
    let st = &mut app.settings;
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
            asked.extend(ito_prefs::theme_page(ui, l, w, themes, (&t, &d, &li), &note, |ui| preview(ui, tr("settings.sample"))));
        }
        MINATO => {
            section(ui, l, tr("settings.rules"), |ui| {
                for r in minato_core::inbox::RULES {
                    let on = st.rules.0[r.index()];
                    if ito_prefs::row(ui, l, tr(r.why()), "", |ui| ito_prefs::switch(ui, l, on)) {
                        st.rules.0[r.index()] = !on;
                    }
                }
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
/// keybar, small.
fn preview(ui: &mut Ui, sample: &str) {
    let c = ito_theme::colors();
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width().min(420.0), 96.0), Sense::hover());
    let p = ui.painter_at(r);
    p.rect_filled(r, 8.0, c.bg);
    p.rect_stroke(r, 8.0, Stroke::new(1.0, c.border), StrokeKind::Inside);
    let inner = r.shrink(12.0);
    let mono = FontId::monospace(12.0);
    p.text(inner.left_top(), Align2::LEFT_TOP, sample, FontId::proportional(12.0), c.dim);
    let mut x = inner.left();
    for (k, name) in ["1 Inbox", "2 List", "3 Actions", "4 Board"].into_iter().enumerate() {
        let t = p.text(egui::pos2(x, inner.top() + 26.0), Align2::LEFT_TOP, name, mono.clone(), if k == 0 { crate::ui::ACCENT } else { c.fg });
        x = t.right() + 14.0;
    }
    p.text(egui::pos2(inner.left(), inner.top() + 52.0), Align2::LEFT_TOP, "● OK   ✗ failed   ▶ running", mono, c.fg);
    p.circle_filled(egui::pos2(inner.right() - 8.0, inner.top() + 8.0), 4.0, c.done);
}
