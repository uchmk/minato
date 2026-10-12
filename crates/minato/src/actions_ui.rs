//! The Actions view (docs/design.md, 「3 Actions」): the filter column, the
//! runs folded by `minato_core::fold`, and the detail of one run with its
//! next step, jobs, the last ten results and artifacts. Painted on rects; no
//! scrollbar anywhere.

use egui::{Align2, Color32, CornerRadius, FontId, Rect, Stroke, Ui, pos2, vec2};
use minato_core::actions::{self, Bucket, Col, Pick, Run, Status};
use minato_core::fold::{Show, fold};
use minato_core::list::{age, epoch};

use crate::app::{App, now_epoch};
use crate::list_ui::{BAD, LINE, OK, PENDING};
use crate::ui::ACCENT;

const FILTER_W: f32 = 210.0;
const ROW_FULL: f32 = 64.0;
const ROW_FOLDED: f32 = 24.0;
const MORE_H: f32 = 22.0;
const HEAD_H: f32 = 30.0;

fn tone(s: Status) -> Color32 {
    let c = ito_theme::colors();
    match s {
        Status::Ok => OK,
        Status::Bad => BAD,
        Status::Running | Status::Queued => PENDING,
        Status::Idle => c.dim,
    }
}

pub fn draw(ui: &mut Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let detail_w = ((r.width() - FILTER_W) * 0.44).max(280.0);
    let filters = Rect::from_min_size(r.min, vec2(FILTER_W, r.height()));
    let detail = Rect::from_min_max(pos2(r.right() - detail_w, r.top()), r.max);
    let runs = Rect::from_min_max(filters.right_top(), detail.left_bottom());
    filter_col(ui, app, filters);
    runs_col(ui, app, runs);
    detail_col(ui, app, detail);
    let p = ui.painter();
    for x in [filters.right(), detail.left()] {
        p.line_segment([pos2(x, r.top()), pos2(x, r.bottom())], Stroke::new(1.0, c.border));
    }
    let on = match app.acts.col {
        Col::Filters => filters,
        Col::Runs => runs,
        Col::Detail => detail,
    };
    p.line_segment([on.left_top(), on.right_top()], Stroke::new(2.0, ACCENT));
}

fn pick_label(app: &App, p: &Pick) -> String {
    let all = || app.tr("list.all").to_string();
    match p {
        Pick::Repo(r) => r.clone().unwrap_or_else(all),
        Pick::State(s) => s.map_or_else(all, |b: Bucket| app.tr(b.word()).to_string()),
        Pick::Workflow(w) => w.clone().unwrap_or_else(all),
    }
}

fn filter_col(ui: &mut Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let p = ui.painter().with_clip_rect(r);
    let groups = actions::options(&app.runs);
    let focus = app.acts.col == Col::Filters;
    // The cursor's row stays in view.
    let mut cursor_y = 10.0;
    let (mut y, mut k) = (10.0, 0);
    for (_, picks) in &groups {
        y += LINE + 6.0;
        for _ in picks {
            if k == app.acts.cursor {
                cursor_y = y;
            }
            y += LINE;
            k += 1;
        }
        y += 6.0;
    }
    let mut y = r.top() + 10.0 - (cursor_y + LINE - r.height()).max(0.0);
    let mut flat = 0;
    for (name, picks) in &groups {
        let word = match *name {
            "repo" => "list.repo",
            "state" => "run.state",
            _ => "run.workflow",
        };
        p.text(pos2(r.left() + 12.0, y + 9.0), Align2::LEFT_CENTER, app.tr(word), FontId::proportional(11.0), c.dim);
        y += LINE + 6.0;
        for pick in picks {
            let row = Rect::from_min_size(pos2(r.left() + 4.0, y), vec2(r.width() - 8.0, LINE));
            if focus && flat == app.acts.cursor {
                p.rect_filled(row, CornerRadius::same(4), ACCENT.gamma_multiply(0.18));
            }
            let chosen = app.acts.is_chosen(pick);
            let color = if chosen { ACCENT } else { c.fg };
            p.text(pos2(row.left() + 8.0, row.center().y), Align2::LEFT_CENTER, if chosen { "●" } else { "○" }, FontId::proportional(10.0), color);
            let count = count(app, pick);
            let t = p.text(pos2(row.right() - 8.0, row.center().y), Align2::RIGHT_CENTER, count.to_string(), FontId::proportional(11.0), c.dim);
            p.with_clip_rect(Rect::from_min_max(row.min, pos2(t.left() - 4.0, row.bottom()))).text(pos2(row.left() + 24.0, row.center().y), Align2::LEFT_CENTER, pick_label(app, pick), FontId::proportional(12.5), color);
            y += LINE;
            flat += 1;
        }
        y += 6.0;
    }
}

/// How many runs a filter choice would show on its own.
fn count(app: &App, pick: &Pick) -> usize {
    let mut only = actions::ActState::default();
    only.apply(pick.clone());
    only.shown(&app.runs).len()
}

fn runs_col(ui: &mut Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let shown = app.acts.shown(&app.runs);
    let head = Rect::from_min_size(r.min, vec2(r.width(), HEAD_H));
    let p = ui.painter().with_clip_rect(r);
    p.text(pos2(head.left() + 12.0, head.center().y), Align2::LEFT_CENTER, query(app), FontId::monospace(11.5), c.dim);
    let n = if app.ja() { format!("{} 件", shown.len()) } else { shown.len().to_string() };
    p.text(pos2(head.right() - 12.0, head.center().y), Align2::RIGHT_CENTER, n, FontId::monospace(11.5), c.fg);
    p.line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0, c.border));
    let body = Rect::from_min_max(head.left_bottom(), r.max);
    if shown.is_empty() {
        let words = if app.viewer.is_none() || app.runs.is_empty() { "list.loading" } else { "list.none" };
        p.text(body.center(), Align2::CENTER_CENTER, app.tr(words), FontId::proportional(13.0), c.dim);
        return;
    }
    let sel = app.acts.sel.min(shown.len() - 1);
    let f = fold(&vec![ROW_FULL; shown.len()], ROW_FOLDED, MORE_H, body.height(), sel);
    let now = now_epoch();
    let focus = app.acts.col == Col::Runs;
    let mut y = body.top();
    if f.above > 0 {
        more(&p, app, body, y, f.above, "list.above");
        y += MORE_H;
    }
    for (k, run) in shown.iter().enumerate() {
        let h = match f.rows[k] {
            Show::Hidden => continue,
            Show::Full => ROW_FULL,
            Show::Folded => ROW_FOLDED,
        };
        let row = Rect::from_min_size(pos2(body.left(), y), vec2(body.width(), h));
        run_row(&p, run, row, f.rows[k] == Show::Full, k == sel, focus, now);
        y += h;
    }
    if f.below > 0 {
        more(&p, app, body, y, f.below, "list.below");
    }
}

/// The query line, like GitHub's: `repo:… status:… workflow:"…"`.
fn query(app: &App) -> String {
    let mut q = vec![];
    if let Some(r) = &app.acts.repo {
        q.push(format!("repo:{r}"));
    }
    if let Some(b) = app.acts.state {
        q.push(format!("status:{}", b.word().trim_start_matches("run.")));
    }
    if let Some(w) = &app.acts.workflow {
        q.push(format!("workflow:\"{w}\""));
    }
    if q.is_empty() {
        q.push(format!("user:{}", app.viewer.as_deref().unwrap_or("")));
    }
    q.join(" ")
}

fn more(p: &egui::Painter, app: &App, body: Rect, y: f32, n: usize, side: &'static str) {
    let c = ito_theme::colors();
    let words = if app.ja() { format!("⋯ {}{} 件", app.tr(side), n) } else { format!("⋯ {} {}", n, app.tr(side)) };
    p.text(pos2(body.left() + 14.0, y + MORE_H / 2.0), Align2::LEFT_CENTER, words, FontId::proportional(11.5), c.dim);
}

fn ago(iso: &str, now: i64) -> String {
    epoch(iso).map(|t| age(iso, now.max(t))).unwrap_or_default()
}

fn run_row(p: &egui::Painter, run: &Run, row: Rect, full: bool, selected: bool, focus: bool, now: i64) {
    let c = ito_theme::colors();
    if selected {
        p.rect_filled(row, 0.0, ACCENT.gamma_multiply(if focus { 0.16 } else { 0.08 }));
        p.rect_filled(Rect::from_min_size(row.min, vec2(3.0, row.height())), 0.0, ACCENT);
    }
    let p = p.with_clip_rect(row);
    let x = row.left() + 14.0;
    let color = tone(run.status);
    p.text(pos2(x, row.top() + 16.0), Align2::LEFT_CENTER, run.status.mark(), FontId::proportional(14.0), color);
    let dur = run.seconds(now).map(actions::duration).unwrap_or_default();
    let t = p.text(pos2(row.right() - 12.0, row.top() + 16.0), Align2::RIGHT_CENTER, dur, FontId::monospace(11.0), c.dim);
    let clip = Rect::from_min_max(row.min, pos2(t.left() - 8.0, row.bottom()));
    p.with_clip_rect(clip).text(pos2(x + 20.0, row.top() + 16.0), Align2::LEFT_CENTER, &run.title, FontId::proportional(13.5), c.fg);
    if !full {
        return;
    }
    let line2 = format!("{}  {}  · {}", run.workflow, run.event, run.branch);
    p.with_clip_rect(clip).text(pos2(x + 20.0, row.top() + 34.0), Align2::LEFT_CENTER, line2, FontId::proportional(11.5), color);
    let line3 = format!("{} · #{} · {} · {}", run.repo, run.number, run.actor, ago(&run.started_at, now));
    p.text(pos2(x + 20.0, row.top() + 51.0), Align2::LEFT_CENTER, line3, FontId::proportional(11.5), c.dim);
}

fn detail_col(ui: &mut Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let p = ui.painter().with_clip_rect(r);
    let Some(run) = app.run() else {
        p.text(r.center(), Align2::CENTER_CENTER, app.tr("detail.pick"), FontId::proportional(13.0), c.dim);
        return;
    };
    let now = now_epoch();
    let w = r.width() - 28.0;
    let x = r.left() + 14.0;
    let mut y = r.top() + 14.0 - app.acts.scroll * LINE;
    let block = |y: &mut f32, text: String, font: FontId, color: Color32| {
        let g = ui.fonts_mut(|f| f.layout(text, font, color, w));
        let h = g.size().y;
        p.galley(pos2(x, *y), g, color);
        *y += h;
    };
    let color = tone(run.status);
    block(&mut y, format!("{} #{}   {} {}", run.workflow, run.number, run.status.mark(), app.tr(run.status.word())), FontId::proportional(12.5), color);
    y += 6.0;
    block(&mut y, run.title.clone(), FontId::proportional(16.0), c.fg);
    y += 6.0;
    let dur = run.seconds(now).map(actions::duration).unwrap_or_default();
    block(&mut y, format!("{} · {} · {} · {} · {} · {}", run.event, run.branch, run.sha, run.actor, dur, ago(&run.started_at, now)), FontId::proportional(11.5), c.dim);
    y += 6.0;
    block(&mut y, run.repo.clone(), FontId::monospace(11.0), c.dim);
    y += 14.0;
    // What to do next.
    let (key, why) = match run.status {
        Status::Bad => ("R", "run.next_bad"),
        Status::Running | Status::Queued => ("X", "run.next_active"),
        Status::Ok | Status::Idle => ("D", "run.next_ok"),
    };
    block(&mut y, format!("[{key}] {}", app.tr(why)), FontId::proportional(12.5), ACCENT);
    y += 4.0;
    if run.status == Status::Bad {
        let bad = actions::history(&app.runs, run, 10).iter().filter(|h| h.status == Status::Bad).count();
        block(&mut y, if app.ja() { format!("直近 10 回のうち失敗は {bad} 回") } else { format!("{bad} of the last 10 runs failed") }, FontId::proportional(11.5), c.dim);
        y += 4.0;
    }
    let mut alt = format!("e {}   o {}   D {}", app.tr("key.logs"), app.tr("key.browser"), app.tr("key.dispatch"));
    if run.pr.is_some() {
        alt.push_str(&format!("   p {}", app.tr("run.open_pr")));
    }
    block(&mut y, alt, FontId::proportional(11.5), c.dim);
    y += 14.0;
    let have = app.run_detail.as_ref().filter(|d| d.id == run.id);
    block(&mut y, app.tr("run.jobs").to_string(), FontId::proportional(11.0), c.dim);
    y += 4.0;
    match have.and_then(|d| d.jobs.as_ref()) {
        None => block(&mut y, app.tr("detail.fetching").to_string(), FontId::proportional(12.0), c.dim),
        Some(jobs) => {
            for j in jobs {
                let d = j.seconds(now).map(actions::duration).unwrap_or_default();
                block(&mut y, format!("{} {}   {d}", j.status.mark(), j.name), FontId::proportional(12.5), tone(j.status));
                y += 2.0;
            }
        }
    }
    y += 12.0;
    // The last ten results, newest at the left.
    block(&mut y, app.tr("run.history").to_string(), FontId::proportional(11.0), c.dim);
    y += 4.0;
    let hist = actions::history(&app.runs, run, 10);
    let top = y;
    for (k, h) in hist.iter().enumerate() {
        let sq = Rect::from_min_size(pos2(x + k as f32 * 18.0, top), vec2(14.0, 14.0));
        p.rect_filled(sq, CornerRadius::same(3), tone(h.status));
    }
    y += 22.0;
    if hist.iter().any(|h| h.status == Status::Bad) {
        let bad = hist.iter().filter(|h| h.status == Status::Bad).count();
        block(&mut y, if app.ja() { format!("落ちたのは {bad} 回") } else { format!("{bad} failed") }, FontId::proportional(11.5), BAD);
    } else if !hist.is_empty() {
        block(&mut y, app.tr("run.all_ok").to_string(), FontId::proportional(11.5), OK);
    }
    y += 14.0;
    if let Some(arts) = have.and_then(|d| d.artifacts.as_ref()).filter(|a| !a.is_empty()) {
        block(&mut y, app.tr("run.artifacts").to_string(), FontId::proportional(11.0), c.dim);
        y += 4.0;
        for a in arts {
            block(&mut y, format!("{}   {}", a.name, actions::size(a.bytes)), FontId::proportional(12.5), c.fg);
            y += 2.0;
        }
    }
}
