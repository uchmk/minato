//! The Actions view (docs/design.md, 「3 Actions」): the filter column, the
//! runs folded by `minato_core::fold`, and the detail of one run with its
//! next step, jobs, the last ten results and artifacts. Painted on rects; no
//! scrollbar anywhere.

use egui::{Align2, Color32, CornerRadius, FontId, Painter, Rect, Stroke, StrokeKind, Ui, pos2, vec2};
use minato_core::actions::{self, Bucket, Col, Pick, Run, Status};
use minato_core::fold::{HEAD, Show, fold_head};
use minato_core::list::{age, epoch};

use crate::app::{App, Click, Pane, Wheel, now_epoch};
use crate::list_ui::LINE;
use crate::style::{self, ACCENT, Next};
use crate::ui::{self, Out};

/// A run, and one folded to its title.
const ROW_FULL: f32 = 76.0;
const ROW_FOLDED: f32 = 28.0;
const MORE_H: f32 = 22.0;
/// The bar with the query.
const HEAD_H: f32 = 40.0;
/// A filter group's heading, and a filter's row.
const GROUP_H: f32 = 30.0;
const ENT_H: f32 = 24.0;

fn tone(s: Status) -> Color32 {
    match s {
        Status::Ok => style::OK,
        Status::Bad => style::BAD,
        Status::Running | Status::Queued => style::RUN,
        Status::Idle => style::IDLE,
    }
}

/// A state filter's mark and colour, as its runs show it.
fn bucket_mark(b: Bucket) -> (&'static str, Color32) {
    match b {
        Bucket::Bad => (Status::Bad.mark(), style::BAD),
        Bucket::Active => (Status::Running.mark(), style::RUN),
        Bucket::Ok => (Status::Ok.mark(), style::OK),
        Bucket::Idle => (Status::Idle.mark(), style::IDLE),
    }
}

pub fn draw(ui: &mut Ui, app: &App, r: Rect, out: &mut Out) {
    for (pane, rect) in ui::panes(&app.acts_split, r) {
        let col = match pane {
            Pane::Left => Col::Filters,
            Pane::Mid => Col::Runs,
            Pane::Right => Col::Detail,
        };
        if ui::hit(ui, rect, ("acts-col", pane)).clicked() {
            out.click = Some(Click::ActCol(col));
        }
        ui::wheel(ui.ctx(), ui.layer_id(), rect, Wheel::Acts(col), out);
        match pane {
            Pane::Left => filter_col(ui, app, rect, out),
            Pane::Mid => runs_col(ui, app, rect, out),
            Pane::Right => detail_col(ui, app, rect),
        }
        if app.acts.col == col {
            ui.painter().line_segment([rect.left_top(), rect.right_top()], Stroke::new(2.0, ACCENT));
        }
    }
    ui::dividers(ui, &app.acts_split, r, out);
}

fn pick_label(app: &App, p: &Pick) -> String {
    let all = || app.tr("list.all").to_string();
    match p {
        Pick::Repo(r) => r.clone().unwrap_or_else(all),
        Pick::State(s) => s.map_or_else(all, |b: Bucket| app.tr(b.word()).to_string()),
        Pick::Workflow(w) => w.clone().unwrap_or_else(all),
    }
}

/// The filters, as the list's: a heading per group, a row per choice with
/// its ● when chosen, the state's mark, and how many runs it has.
fn filter_col(ui: &Ui, app: &App, r: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(r, 0.0, c.side);
    let groups = actions::options(&app.runs);
    let focus = app.acts.col == Col::Filters;
    // The cursor's row stays in view.
    let mut cursor_y = 0.0;
    let (mut yy, mut k) = (6.0, 0);
    for (_, picks) in &groups {
        yy += GROUP_H;
        for _ in picks {
            if k == app.acts.cursor {
                cursor_y = yy;
            }
            yy += ENT_H;
            k += 1;
        }
    }
    let mut y = r.top() + 6.0 - (cursor_y + ENT_H + 16.0 - r.height()).max(0.0);
    let mut flat = 0;
    for (name, picks) in &groups {
        let word = match *name {
            "repo" => "list.repo",
            "state" => "run.state",
            _ => "run.workflow",
        };
        style::bold(&p, pos2(r.left() + 14.0, y + 12.0 + 7.0), Align2::LEFT_CENTER, app.tr(word), FontId::proportional(11.0), c.dim);
        y += GROUP_H;
        for pick in picks {
            let row = Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), ENT_H));
            let resp = ui::hit(ui, row, ("acts-filter", flat));
            if resp.clicked() {
                out.click = Some(Click::ActFilter(flat));
            }
            if focus && flat == app.acts.cursor {
                p.rect_filled(row, 0.0, style::sel());
                p.rect_filled(Rect::from_min_size(row.min, vec2(3.0, ENT_H)), 0.0, ACCENT);
            } else if resp.hovered() {
                p.rect_filled(row, 0.0, c.hover());
            }
            let cy = row.center().y;
            if app.acts.is_chosen(pick) {
                p.text(pos2(row.left() + 14.0, cy), Align2::LEFT_CENTER, "●", FontId::proportional(9.0), ACCENT);
            }
            let t = p.text(pos2(row.right() - 14.0, cy), Align2::RIGHT_CENTER, count(app, pick).to_string(), FontId::monospace(11.5), c.dim);
            let mut x = row.left() + 14.0 + 18.0;
            if let Pick::State(Some(b)) = pick {
                let (m, col) = bucket_mark(*b);
                x = p.text(pos2(x, cy), Align2::LEFT_CENTER, m, FontId::proportional(12.5), col).right() + 6.0;
            }
            let g = style::truncated(&p, pick_label(app, pick), FontId::proportional(13.0), c.fg, t.left() - 6.0 - x);
            p.galley(pos2(x, cy - g.size().y / 2.0), g, c.fg);
            y += ENT_H;
            flat += 1;
        }
    }
}

/// How many runs a filter choice would show on its own.
fn count(app: &App, pick: &Pick) -> usize {
    let mut only = actions::ActState::default();
    only.apply(pick.clone());
    only.shown(&app.runs).len()
}

/// The query and the count over the runs, each folded to fit.
fn runs_col(ui: &Ui, app: &App, r: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let shown = app.acts.shown(&app.runs);
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(r, 0.0, c.side);
    let head = Rect::from_min_size(r.min, vec2(r.width(), HEAD_H));
    let n = p.text(pos2(head.right() - 12.0, head.center().y), Align2::RIGHT_CENTER, app.tr("list.count").replace("{}", &shown.len().to_string()), FontId::proportional(12.0), c.dim);
    let q = Rect::from_min_max(pos2(head.left() + 12.0, head.center().y - 11.0), pos2(n.left() - 8.0, head.center().y + 11.0));
    p.rect_filled(q, CornerRadius::same(6), c.bg);
    p.rect_stroke(q, CornerRadius::same(6), Stroke::new(1.0, c.border), StrokeKind::Inside);
    let g = style::truncated(&p, query(app), FontId::monospace(12.0), c.fg, q.width() - 18.0);
    p.galley(pos2(q.left() + 9.0, q.center().y - g.size().y / 2.0), g, c.fg);
    p.line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0, c.border));
    let body = Rect::from_min_max(head.left_bottom(), r.max);
    if shown.is_empty() {
        let words = if app.viewer.is_none() || app.runs.is_empty() { "list.loading" } else { "list.none" };
        p.text(pos2(body.left() + 14.0, body.top() + 20.0), Align2::LEFT_CENTER, app.tr(words), FontId::proportional(12.5), c.dim);
        return;
    }
    let sel = app.acts.sel.min(shown.len() - 1);
    let f = fold_head(&vec![ROW_FULL; shown.len()], ROW_FOLDED, MORE_H, body.height(), sel, HEAD);
    let now = now_epoch();
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
        let resp = ui::hit(ui, row, ("acts-run", k));
        if resp.clicked() {
            out.click = Some(Click::ActRun(k));
        }
        run_row(&p, run, row, f.rows[k] == Show::Full, k == sel, resp.hovered(), now);
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

fn more(p: &Painter, app: &App, body: Rect, y: f32, n: usize, side: &'static str) {
    style::more(p, app.ja(), app.tr(side), body.left() + 14.0, y + MORE_H / 2.0, n);
}

fn ago(iso: &str, now: i64) -> String {
    epoch(iso).map(|t| age(iso, now.max(t))).unwrap_or_default()
}

/// One run: its state's mark; the title; the workflow, the event and the
/// branch; where, who and when; the duration at the right.
fn run_row(p: &Painter, run: &Run, row: Rect, full: bool, on: bool, hover: bool, now: i64) {
    let c = ito_theme::colors();
    if on {
        p.rect_filled(row, 0.0, style::sel());
        p.rect_filled(Rect::from_min_size(row.min, vec2(3.0, row.height())), 0.0, ACCENT);
    } else if hover {
        p.rect_filled(row, 0.0, c.hover());
    }
    p.line_segment([row.left_bottom(), row.right_bottom()], Stroke::new(1.0, c.border));
    let p = p.with_clip_rect(row);
    let y1 = row.top() + if full { 8.0 } else { 5.0 } + 9.0;
    p.text(pos2(row.left() + 14.0 + 8.0, y1), Align2::CENTER_CENTER, run.status.mark(), FontId::proportional(13.0), tone(run.status));
    let x = row.left() + 14.0 + 16.0 + 10.0;
    let right = row.right() - 14.0;
    let dur = run.seconds(now).map(actions::duration).unwrap_or_default();
    let t = p.text(pos2(right, y1), Align2::RIGHT_CENTER, dur, FontId::proportional(12.0), c.dim);
    let rx = t.left() - 10.0;
    let g = style::truncated(&p, run.title.clone(), FontId::proportional(13.0), c.fg, rx - x);
    let ty = y1 - g.size().y / 2.0;
    p.galley(pos2(x + 0.45, ty), g.clone(), c.fg);
    p.galley(pos2(x, ty), g, c.fg);
    if !full {
        return;
    }
    let y2 = y1 + 9.0 + 4.0 + 9.0;
    let wx = style::chip(&p, x, y2, &run.workflow, FontId::monospace(11.0), 6.0, 17.0, 4, c.border, style::head(), c.fg) + 6.0;
    let g = style::truncated(&p, format!("{}  · {}", run.event, run.branch), FontId::monospace(11.0), c.dim, rx - wx);
    p.galley(pos2(wx, y2 - g.size().y / 2.0), g, c.dim);
    let y3 = y2 + 9.0 + 3.0 + 8.0;
    let g = style::truncated(&p, format!("{} · #{} · {} · {}", run.repo, run.number, run.actor, ago(&run.started_at, now)), FontId::proportional(12.0), c.dim, right - x);
    p.galley(pos2(x, y3 - g.size().y / 2.0), g, c.dim);
}

/// The run as the mock's detail draws it: where and what state; the title
/// and how it went; the next step and the other keys; the jobs, the last ten
/// results and the artifacts.
fn detail_col(ui: &Ui, app: &App, r: Rect) {
    let c = ito_theme::colors();
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(r, 0.0, c.bg);
    let Some(run) = app.run() else {
        p.text(r.center(), Align2::CENTER_CENTER, app.tr("detail.pick"), FontId::proportional(13.0), c.dim);
        return;
    };
    let now = now_epoch();
    let x = r.left() + 26.0;
    let w = (r.width() - 52.0).min(900.0);
    let mut y = r.top() + 18.0 - app.acts.scroll * LINE + 10.0;
    // The head: the repository, Run, the workflow and its number, the state.
    let mut hx = style::tag(&p, x, y, &run.repo, &run.repo) + 8.0;
    hx = style::run_chip(&p, hx, y, "▶ Run", ACCENT) + 8.0;
    hx = p.text(pos2(hx, y), Align2::LEFT_CENTER, format!("{} #{}", run.workflow, run.number), FontId::monospace(12.5), c.fg).right() + 8.0;
    let color = tone(run.status);
    let words = format!("{} {}", run.status.mark(), app.tr(run.status.word()));
    style::chip(&p, hx, y, &words, FontId::proportional(11.5), 7.0, 18.0, 9, color, color.gamma_multiply(0.14), color);
    y += 10.0 + 10.0;
    let title = p.layout(run.title.clone(), FontId::proportional(19.0), c.fg, w);
    let th = title.size().y;
    p.galley(pos2(x + 0.5, y), title.clone(), c.fg);
    p.galley(pos2(x, y), title, c.fg);
    y += th + 8.0;
    let dur = run.seconds(now).map(actions::duration).unwrap_or_default();
    let branch = if run.pr.is_some() { format!("{} → main", run.branch) } else { run.branch.clone() };
    let mut mx = x;
    for m in [run.event.clone(), branch, run.sha.clone(), run.actor.clone(), dur, ago(&run.started_at, now)] {
        if !m.is_empty() {
            mx = p.text(pos2(mx, y + 8.0), Align2::LEFT_CENTER, m, FontId::monospace(12.0), c.dim).right() + 14.0;
        }
    }
    y += 16.0;
    // What to do next: re-run a failure, stop a run that goes.
    let hist = actions::history(&app.runs, run, 10);
    let bad = hist.iter().filter(|h| h.status == Status::Bad).count();
    let note = if app.ja() { format!("このワークフローは最近 10 回で {bad} 回落ちている") } else { format!("{bad} of the last 10 runs of this workflow failed") };
    let next = match run.status {
        Status::Bad => Some(Next { tone: style::BAD, lead: app.tr("detail.next"), label: app.tr("run.next_bad"), note: &note, key: Some("R") }),
        Status::Running | Status::Queued => Some(Next { tone: style::RUN, lead: app.tr("detail.next"), label: app.tr("run.next_active"), note: "", key: Some("X") }),
        Status::Ok | Status::Idle => None,
    };
    if let Some(n) = next {
        y = style::next_box(&p, x, y, w, &n);
    } else {
        y += 12.0;
    }
    let mut keys = vec![("e", app.tr("key.logs"))];
    if run.pr.is_some() {
        keys.push(("p", app.tr("run.open_pr")));
    }
    keys.extend([("D", app.tr("key.dispatch")), ("o", app.tr("key.browser"))]);
    y = style::alt_keys(&p, x, y, x + w, &keys);
    let have = app.run_detail.as_ref().filter(|d| d.id == run.id);
    y = style::heading(&p, x, y, app.tr("run.jobs"));
    match have.and_then(|d| d.jobs.as_ref()) {
        None => {
            p.text(pos2(x, y + 9.0), Align2::LEFT_CENTER, app.tr("detail.fetching"), FontId::proportional(12.5), c.dim);
            y += 18.0;
        }
        Some(jobs) => match have.and_then(|d| d.workflow.as_ref()).filter(|w| w.layers().iter().all(|&l| l < MAX_COLS) && w.jobs.iter().any(|j| !j.needs.is_empty())) {
            Some(wf) => y += graph(&p, jobs, wf, pos2(x, y), w, now),
            None => {
                for j in jobs {
                    let d = j.seconds(now).map(actions::duration).unwrap_or_default();
                    y = style::check_row(&p, x, y, w, j.status.mark(), tone(j.status), &j.name, &d) + 2.0;
                }
            }
        },
    }
    // The last ten results, newest at the left.
    y = style::heading(&p, x, y, app.tr("run.history"));
    for (k, h) in hist.iter().enumerate() {
        let sq = Rect::from_min_size(pos2(x + 10.0 + k as f32 * 18.0, y), vec2(14.0, 14.0));
        p.rect_filled(sq, CornerRadius::same(3), tone(h.status));
    }
    let hx = x + 10.0 + hist.len() as f32 * 18.0 + 8.0;
    if bad > 0 {
        p.text(pos2(hx, y + 7.0), Align2::LEFT_CENTER, if app.ja() { format!("落ちたのは {bad} 回") } else { format!("{bad} failed") }, FontId::proportional(12.0), c.dim);
    } else if !hist.is_empty() {
        p.text(pos2(hx, y + 7.0), Align2::LEFT_CENTER, app.tr("run.all_ok"), FontId::proportional(12.0), c.dim);
    }
    y += 14.0;
    if let Some(arts) = have.and_then(|d| d.artifacts.as_ref()).filter(|a| !a.is_empty()) {
        y = style::heading(&p, x, y, app.tr("run.artifacts"));
        for a in arts {
            let cy = y + 10.0;
            let m = p.text(pos2(x + 10.0, cy), Align2::LEFT_CENTER, "▤", FontId::monospace(12.0), c.fg);
            p.text(pos2(m.right() + 10.0, cy), Align2::LEFT_CENTER, &a.name, FontId::monospace(12.0), c.fg);
            p.text(pos2(x + w - 10.0, cy), Align2::RIGHT_CENTER, actions::size(a.bytes), FontId::monospace(12.0), c.dim);
            y += 20.0;
        }
    }
}

/// Most columns the `needs:` graph is drawn with; deeper workflows are listed.
const MAX_COLS: usize = 4;
const BOX_H: f32 = 36.0;
const BOX_GAP: f32 = 8.0;
const COL_GAP: f32 = 18.0;

/// The jobs as boxes in columns by how deep their `needs:` go, joined by
/// lines. Returns the height used.
fn graph(p: &Painter, jobs: &[minato_core::actions::Job], wf: &minato_core::workflow::Workflow, at: egui::Pos2, w: f32, now: i64) -> f32 {
    let c = ito_theme::colors();
    let layers = wf.layers();
    let src: Vec<Option<usize>> = jobs.iter().map(|j| wf.source_of(j)).collect();
    let col_of = |k: usize| src[k].map_or(0, |s| layers[s]);
    let cols = (0..jobs.len()).map(col_of).max().unwrap_or(0) + 1;
    let bw = (w - COL_GAP * (cols - 1) as f32) / cols as f32;
    let mut rows = vec![0usize; cols];
    let mut rect = Vec::with_capacity(jobs.len());
    for k in 0..jobs.len() {
        let col = col_of(k);
        let r = Rect::from_min_size(pos2(at.x + col as f32 * (bw + COL_GAP), at.y + rows[col] as f32 * (BOX_H + BOX_GAP)), vec2(bw, BOX_H));
        rows[col] += 1;
        rect.push(r);
    }
    for (k, s) in src.iter().enumerate() {
        let Some(s) = s else { continue };
        for need in &wf.jobs[*s].needs {
            let Some(from) = wf.jobs.iter().position(|j| &j.key == need) else { continue };
            for (m, r) in rect.iter().enumerate() {
                if src[m] == Some(from) {
                    let (a, b) = (r.right_center(), rect[k].left_center());
                    let mid = a.x + (b.x - a.x) / 2.0;
                    p.add(egui::Shape::line(vec![a, pos2(mid, a.y), pos2(mid, b.y), b], Stroke::new(1.0, c.dim)));
                }
            }
        }
    }
    for (job, r) in jobs.iter().zip(&rect) {
        let t = tone(job.status);
        p.rect_filled(*r, CornerRadius::same(5), t.gamma_multiply(0.16));
        p.rect_stroke(*r, CornerRadius::same(5), Stroke::new(1.0, t), egui::StrokeKind::Inside);
        let clip = p.with_clip_rect(r.shrink(3.0));
        clip.text(pos2(r.left() + 7.0, r.top() + 12.0), Align2::LEFT_CENTER, format!("{} {}", job.status.mark(), job.name), FontId::proportional(11.5), c.fg);
        let d = job.seconds(now).map(actions::duration).unwrap_or_default();
        clip.text(pos2(r.left() + 7.0, r.top() + 27.0), Align2::LEFT_CENTER, d, FontId::monospace(10.5), c.dim);
    }
    let deepest = rows.iter().copied().max().unwrap_or(0);
    deepest as f32 * (BOX_H + BOX_GAP)
}
