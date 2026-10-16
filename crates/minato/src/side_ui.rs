//! The detail beside the status and security views, drawn like the Actions
//! detail: where and what state, the title, the facts in a row, the next
//! step, the other keys, then sections (checks, details, files, notes).
//! Painted on rects and folded by clipping (no scrollbar).

use egui::{pos2, Align2, Color32, FontId, Rect, Ui};
use minato_core::list::{age, epoch, Label};
use minato_core::overview::{Health, Row};
use minato_core::security::{Alert, Severity};

use crate::app::{now_epoch, App, Click};
use crate::style::{self, Next};
use crate::ui::Out;

/// The widest the text runs, as in the Actions detail.
const MAX_W: f32 = 900.0;
/// The width of the labels' column in the details.
const LABEL_W: f32 = 120.0;
/// Most lines of text a section shows.
const TEXT_LINES: usize = 14;

/// A step the owner can take, with the key of its button.
pub struct Step {
    pub tone: Color32,
    pub label: String,
    pub note: String,
    pub key: char,
}

/// One row of the checks.
pub struct Check {
    pub mark: &'static str,
    pub tone: Color32,
    pub name: String,
    pub time: String,
}

pub enum Section {
    Checks(String, Vec<Check>),
    Facts(String, Vec<(String, String)>),
    Files(String, Vec<(String, u64)>),
    Text(String, Vec<String>),
}

/// What the detail says.
pub struct Sheet {
    pub repo: String,
    /// The chip after the repository (`▶ Lane`).
    pub kind: (String, Color32),
    /// The mono word after it (a number, a tag, a lane's name).
    pub id: String,
    /// The state at the right of the head.
    pub state: (String, Color32),
    pub title: String,
    pub meta: Vec<String>,
    pub labels: Vec<Label>,
    pub next: Option<Step>,
    /// The other keys (`(character, words)`).
    pub keys: Vec<(&'static str, &'static str)>,
    pub sections: Vec<Section>,
}

pub fn draw(ui: &mut Ui, app: &App, r: Rect, out: &mut Out, sheet: Option<Sheet>) {
    let c = ito_theme::colors();
    ui.painter().rect_filled(r, 0.0, c.bg);
    let p = ui.painter().with_clip_rect(r);
    let Some(s) = sheet else {
        p.text(r.center(), Align2::CENTER_CENTER, app.tr("detail.pick"), FontId::proportional(13.0), c.dim);
        return;
    };
    let x = r.left() + 26.0;
    let w = (r.width() - 52.0).clamp(40.0, MAX_W);
    let mut y = r.top() + 28.0;
    // The head: the repository, what it is, its name, the state.
    let mut hx = if s.repo.is_empty() { x } else { style::tag(&p, x, y, &s.repo, &s.repo) + 8.0 };
    if !s.kind.0.is_empty() {
        hx = style::run_chip(&p, hx, y, &s.kind.0, s.kind.1) + 8.0;
    }
    if !s.id.is_empty() {
        hx = p.text(pos2(hx, y), Align2::LEFT_CENTER, &s.id, FontId::monospace(13.5), c.fg).right() + 8.0;
    }
    let color = s.state.1;
    // The state on a line of its own when it would run past the right.
    if hx + p.layout_no_wrap(s.state.0.clone(), FontId::proportional(11.5), color).size().x + 14.0 > x + w {
        (hx, y) = (x, y + 26.0);
    }
    if !s.state.0.is_empty() {
        style::chip(&p, hx, y, &s.state.0, FontId::proportional(11.5), 7.0, 18.0, 9, color, color.gamma_multiply(0.14), color);
    }
    y += 20.0;
    let title = p.layout(s.title.clone(), FontId::proportional(21.0), c.fg, w);
    let th = title.size().y;
    p.galley(pos2(x + 0.5, y), title.clone(), c.fg);
    p.galley(pos2(x, y), title, c.fg);
    y += th + 8.0;
    // The facts in a row, wrapping before one would run past the right.
    let mut mx = x;
    for m in s.meta.iter().filter(|m| !m.is_empty()) {
        let g = p.layout_no_wrap(m.clone(), FontId::monospace(13.0), c.dim);
        if mx > x && mx + g.size().x > x + w {
            (mx, y) = (x, y + 20.0);
        }
        let gw = g.size().x;
        p.galley(pos2(mx, y + 8.0 - g.size().y / 2.0), g, c.dim);
        mx += gw + 14.0;
    }
    y += 16.0;
    if !s.labels.is_empty() {
        y += 6.0;
        style::labels(&p, x, y + 9.0, &s.labels, x + w);
        y += 20.0;
    }
    match &s.next {
        Some(n) => {
            let key = app.char_label(crate::keymap::Ctx::View(app.view), &n.key.to_string());
            let next = Next { tone: n.tone, lead: app.tr("detail.next"), label: &n.label, note: &n.note, key: Some(key.as_str()) };
            let (under, button) = style::next_box(&p, x, y, w, &next);
            y = under;
            if let Some(k) = style::next_click(ui, button, Some(n.key), r) {
                out.click = Some(Click::Key(k));
            }
        }
        None => y += 12.0,
    }
    y = style::alt_keys(&p, x, y, x + w, &app.alt_labels(&s.keys));
    for sec in &s.sections {
        if y > r.bottom() {
            break;
        }
        y = section(&p, sec, x, y, w);
    }
}

fn section(p: &egui::Painter, sec: &Section, x: f32, y: f32, w: f32) -> f32 {
    let c = ito_theme::colors();
    match sec {
        Section::Checks(head, rows) => {
            let mut y = style::heading(p, x, y, head);
            for ch in rows {
                y = style::check_row(p, x, y, w, ch.mark, ch.tone, &ch.name, &ch.time) + 2.0;
            }
            y
        }
        Section::Facts(head, rows) => {
            let mut y = style::heading(p, x, y, head);
            for (label, value) in rows.iter().filter(|(_, v)| !v.is_empty()) {
                let gl = p.layout(label.clone(), FontId::proportional(13.0), c.dim, LABEL_W - 8.0);
                let gv = p.layout(value.clone(), FontId::proportional(13.5), c.fg, (w - LABEL_W - 10.0).max(20.0));
                let h = gv.size().y.max(gl.size().y);
                p.galley(pos2(x + 10.0, y + 1.0), gl, c.dim);
                p.galley(pos2(x + 10.0 + LABEL_W, y), gv, c.fg);
                y += h + 6.0;
            }
            y
        }
        Section::Files(head, files) => {
            let mut y = style::heading(p, x, y, head);
            for (name, bytes) in files {
                crate::actions_ui::artifact(p, x, y + 10.0, w, name, *bytes);
                y += 22.0;
            }
            y
        }
        Section::Text(head, lines) => {
            let mut y = style::heading(p, x, y, head);
            for l in lines {
                if l.is_empty() {
                    y += 6.0;
                    continue;
                }
                let g = p.layout(l.clone(), FontId::proportional(13.5), c.fg, w - 10.0);
                let h = g.size().y;
                p.galley(pos2(x + 10.0, y), g, c.fg);
                y += h + 3.0;
            }
            y
        }
    }
}

/// How long ago `stamp` was, or nothing.
fn ago(stamp: &str, now: i64) -> String {
    epoch(stamp).map(|t| age(stamp, now.max(t))).unwrap_or_default()
}

/// Notes or a body as plain lines: Markdown marks and table rows dropped,
/// runs of blank lines made one, at most `TEXT_LINES` lines.
pub fn plain(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.lines() {
        let t = raw.trim();
        if t.starts_with('|') || t.starts_with("```") || t.starts_with("---") {
            continue;
        }
        let t = t.trim_start_matches('#').trim_start();
        let t = match t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            Some(rest) => format!("• {rest}"),
            None => t.to_string(),
        };
        let t = t.replace("**", "").replace('`', "");
        if t.is_empty() && out.last().is_none_or(String::is_empty) {
            continue;
        }
        out.push(t);
        if out.len() >= TEXT_LINES {
            break;
        }
    }
    while out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    out
}

fn tone(s: Severity) -> Color32 {
    match s {
        Severity::Critical => style::BAD,
        Severity::High => style::RUN,
        Severity::Medium => style::ASK,
        Severity::Low | Severity::Unknown => style::IDLE,
    }
}

fn empty(keys: Vec<(&'static str, &'static str)>) -> Sheet {
    Sheet {
        repo: String::new(),
        kind: (String::new(), style::IDLE),
        id: String::new(),
        state: (String::new(), style::IDLE),
        title: String::new(),
        meta: Vec::new(),
        labels: Vec::new(),
        next: None,
        keys,
        sections: Vec::new(),
    }
}

/// The detail of the alert under the cursor in the security view.
pub fn alert(app: &App, a: &Alert) -> Sheet {
    let now = now_epoch();
    let t = tone(a.severity);
    let mut facts: Vec<(String, String)> = vec![(app.tr("side.place").into(), a.place.clone())];
    for (key, value) in &a.extra {
        let value = match value.as_str() {
            "yes" => app.tr("side.yes").to_string(),
            "no" => app.tr("side.no").to_string(),
            v => v.to_string(),
        };
        facts.push((app.tr(key).into(), value));
    }
    facts.push((app.tr("side.link").into(), a.url.clone()));
    let note = match a.extra.iter().find(|(k, _)| *k == "side.fix") {
        Some((_, v)) => app.tr("side.n_fix").replace("{}", v),
        None => app.tr("side.n_alert").to_string(),
    };
    let mut sections = Vec::new();
    if !a.summary.is_empty() && a.summary != a.what {
        sections.push(Section::Text(app.tr("side.h_summary").into(), vec![a.summary.clone()]));
    }
    sections.push(Section::Facts(app.tr("side.h_facts").into(), facts));
    let body = plain(&a.body);
    if !body.is_empty() {
        sections.push(Section::Text(app.tr("side.h_desc").into(), body));
    }
    Sheet {
        repo: a.repo.clone(),
        kind: (format!("▶ {}", app.tr(a.source.label())), style::ACCENT),
        id: format!("#{}", a.number),
        state: (format!("● {}", app.tr(a.severity.label())), t),
        title: a.what.clone(),
        meta: vec![a.place.clone(), ago(&a.created_at, now)],
        labels: Vec::new(),
        next: Some(Step { tone: t, label: app.tr("side.n_dismiss").into(), note, key: 'x' }),
        keys: vec![
            ("o", app.tr("side.a_open")),
            ("y", app.tr("side.a_copy")),
            ("u", app.tr("side.a_reopen")),
            ("/", app.tr("side.a_find")),
        ],
        sections,
    }
}

/// The detail of the row under the cursor in the status view.
pub fn status(app: &App, row: &Row) -> Sheet {
    let now = now_epoch();
    let keys = vec![("o", app.tr("side.a_open")), ("y", app.tr("side.a_copy")), ("\n", app.tr("side.a_list"))];
    match row {
        Row::Lane(l) => {
            let health = l.health(now);
            let (t, state) = match health {
                Health::Ok => (style::OK, "side.ok"),
                Health::Late => (style::RUN, "side.late"),
                Health::Bad => (style::BAD, "side.bad"),
            };
            let fired = l.fired.map(|f| minato_core::status::ago(Some((now - f).max(0) as u64), app.ja()));
            let step = if l.stalled {
                Some((style::BAD, "side.n_stalled", app.tr("side.n_issue").to_string()))
            } else if l.failed > 0 {
                Some((style::BAD, "side.n_failed", app.tr("side.n_failed_note").replace("{}", &l.failed.to_string())))
            } else if health == Health::Late {
                Some((style::RUN, "side.n_late", app.tr("side.n_issue").to_string()))
            } else {
                None
            };
            let clean = l.worktree.is_empty() || l.worktree.to_lowercase().starts_with("clean");
            let check = |ok: bool, warn: bool, name: &'static str, time: String| Check {
                mark: if ok {
                    "✓"
                } else if warn {
                    "!"
                } else {
                    "✗"
                },
                tone: if ok {
                    style::OK
                } else if warn {
                    style::RUN
                } else {
                    style::BAD
                },
                name: app.tr(name).into(),
                time,
            };
            let checks = vec![
                check(health != Health::Late, true, "side.c_fired", fired.clone().unwrap_or_else(|| app.tr("status.never").into())),
                check(l.failed == 0, false, "side.c_nofail", if l.failed > 0 { l.failed.to_string() } else { String::new() }),
                check(!l.stalled, false, "side.c_nostall", String::new()),
                check(clean, true, "side.c_clean", String::new()),
            ];
            Sheet {
                repo: l.repo.clone(),
                kind: (format!("▶ {}", app.tr("side.k_lane")), style::ACCENT),
                id: l.name.clone(),
                state: (format!("● {}", app.tr(state)), t),
                title: if l.outcome.is_empty() { app.tr("status.never").into() } else { l.outcome.clone() },
                meta: vec![fired.unwrap_or_default(), l.script.clone()],
                labels: Vec::new(),
                next: step.map(|(tone, label, note)| Step { tone, label: app.tr(label).into(), note, key: 'o' }),
                keys,
                sections: vec![
                    Section::Checks(app.tr("side.h_checks").into(), checks),
                    Section::Facts(
                        app.tr("side.h_facts").into(),
                        vec![
                            (app.tr("side.script").into(), l.script.clone()),
                            (app.tr("side.worktree").into(), l.worktree.clone()),
                            (app.tr("side.link").into(), l.url.clone()),
                        ],
                    ),
                ],
            }
        }
        Row::Release { repo, shipped, ahead } => {
            let kind = (format!("▣ {}", app.tr("side.k_release")), style::ACCENT);
            let Some(s) = shipped else {
                return Sheet { repo: repo.clone(), kind, state: (app.tr("side.none").into(), style::IDLE), ..empty(keys) };
            };
            let behind = ahead.filter(|&n| n > 0);
            let ahead_words = ahead.map(|n| app.tr("status.ahead").replace("{}", &n.to_string()));
            let no_sums = s.lacks_sums();
            let t = if no_sums { style::RUN } else { style::OK };
            let state = if s.prerelease {
                "side.prerelease"
            } else if no_sums {
                "side.nosums"
            } else {
                "side.published"
            };
            let step = if no_sums {
                Some(Step { tone: style::RUN, label: app.tr("side.n_nosums").into(), note: app.tr("side.n_nosums_note").into(), key: 'o' })
            } else {
                behind.map(|n| Step {
                    tone: style::ACCENT,
                    label: app.tr("side.n_ahead").into(),
                    note: app.tr("status.ahead").replace("{}", &n.to_string()),
                    key: 'o',
                })
            };
            let check = |ok: bool, name: &'static str, time: String| Check {
                mark: if ok { "✓" } else { "!" },
                tone: if ok { style::OK } else { style::RUN },
                name: app.tr(name).into(),
                time,
            };
            let mut sections = vec![Section::Checks(
                app.tr("side.h_checks").into(),
                vec![
                    check(s.files > 0, "side.c_files", s.files.to_string()),
                    check(!no_sums, "side.c_sums", String::new()),
                    check(behind.is_none(), "side.c_uptodate", ahead_words.clone().unwrap_or_default()),
                ],
            )];
            if !s.assets.is_empty() {
                sections.push(Section::Files(format!("{} · {}", app.tr("run.release"), s.tag), s.assets.clone()));
            }
            let notes = plain(&s.notes);
            if !notes.is_empty() {
                sections.push(Section::Text(app.tr("side.h_notes").into(), notes));
            }
            Sheet {
                repo: repo.clone(),
                kind,
                id: s.tag.clone(),
                state: (format!("● {}", app.tr(state)), t),
                title: if s.name.is_empty() { s.tag.clone() } else { s.name.clone() },
                meta: vec![ago(&s.published_at, now), s.author.clone(), ahead_words.unwrap_or_default()],
                next: step,
                keys,
                sections,
                ..empty(Vec::new())
            }
        }
        Row::Item(i, label) => {
            let Some(it) = app.items.get(*i) else { return empty(keys) };
            let (mark, kt) = style::kind_mark(it.kind);
            let word = match it.kind {
                minato_core::list::Kind::Issue => "Issue",
                minato_core::list::Kind::Pr => "PR",
                minato_core::list::Kind::Discussion => "Discussion",
            };
            let (t, step) = match *label {
                "needs-pc" => (style::RUN, "side.n_pc"),
                "question" => (style::ASK, "side.n_question"),
                _ => (style::ACCENT, "side.n_review"),
            };
            let mut sections = Vec::new();
            let body = plain(&it.peek);
            if !body.is_empty() {
                sections.push(Section::Text(app.tr("side.h_body").into(), body));
            }
            sections.push(Section::Facts(
                app.tr("side.h_facts").into(),
                vec![
                    (app.tr("side.author").into(), it.author.clone()),
                    (app.tr("side.updated").into(), ago(&it.updated_at, now)),
                    (app.tr("side.link").into(), it.url.clone()),
                ],
            ));
            let comments = if it.comments > 0 { app.tr("side.n_comments").replace("{}", &it.comments.to_string()) } else { String::new() };
            Sheet {
                repo: it.repo.clone(),
                kind: (format!("{mark} {word}"), kt),
                id: format!("#{}", it.number),
                state: (format!("● {label}"), t),
                title: it.title.clone(),
                meta: vec![it.author.clone(), ago(&it.updated_at, now), comments],
                labels: it.labels.clone(),
                next: Some(Step { tone: t, label: app.tr(step).into(), note: String::new(), key: 'o' }),
                keys,
                sections,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_read_as_plain_lines() {
        let notes = "## Changes\n\n\n- **Fast** start\n* `x` works\n\n| file | sum |\n|---|---|\n| a.zip | 12 |\n```\ncode\n```\nEnd";
        assert_eq!(plain(notes), ["Changes", "", "• Fast start", "• x works", "", "code", "End"]);
        assert!(plain("\n\n").is_empty());
        let long = (0..40).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n");
        assert_eq!(plain(&long).len(), TEXT_LINES);
    }
}
