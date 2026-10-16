//! The Actions view (docs/design.md, 「3 Actions」): the filter column, the
//! runs folded by `minato_core::fold`, and the detail of one run with its
//! next step, jobs, the last ten results, artifacts and a release's files.
//! Painted on rects; no scrollbar anywhere.

use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Rect, Stroke, StrokeKind, Ui};
use minato_core::actions::{self, Bucket, Col, Pick, Run, Status};
use minato_core::fold::{fold, Show};
use minato_core::list::{age, epoch};

use crate::app::{now_epoch, App, Click, Pane, Wheel};
use crate::list_ui::LINE;
use crate::style::{self, Next, ACCENT};
use crate::ui::{self, Out};

/// A run.
const ROW_FULL: f32 = 76.0;
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
        Bucket::Seen => (Status::Ok.mark(), style::IDLE),
    }
}

pub fn draw(ui: &mut Ui, app: &App, r: Rect, out: &mut Out) {
    for (pane, rect) in ui::panes(&app.acts_split, r) {
        let col = match pane {
            Pane::Left => Col::Filters,
            Pane::Mid => Col::Runs,
            Pane::Right => Col::Detail,
        };
        // A click focuses the column; no hand, as nothing is pressed.
        if ui.interact(rect, ui.id().with(("acts-col", pane)), egui::Sense::click()).clicked() {
            out.click = Some(Click::ActCol(col));
        }
        ui::wheel(ui.ctx(), ui.layer_id(), rect, Wheel::Acts(col), out);
        match pane {
            Pane::Left => filter_col(ui, app, rect, out),
            Pane::Mid => runs_col(ui, app, rect, out),
            Pane::Right => detail_col(ui, app, rect, out),
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
    // Only whole rows; an end with rows past it says how many.
    let lift = if yy + 10.0 > r.height() { (cursor_y + ENT_H + 16.0 + MORE_H - r.height()).max(0.0) } else { 0.0 };
    let top = r.top() + if lift > 0.0 { MORE_H } else { 0.0 };
    let bottom = r.bottom() - if r.top() + 6.0 - lift + yy > r.bottom() { MORE_H } else { 0.0 };
    let (mut above, mut below) = (0, 0);
    let mut y = r.top() + 6.0 - lift;
    let mut flat = 0;
    for (name, picks) in &groups {
        let word = match *name {
            "repo" => "list.repo",
            "state" => "run.state",
            _ => "run.workflow",
        };
        if y >= top && y + GROUP_H <= bottom {
            style::bold(&p, pos2(r.left() + 14.0, y + 12.0 + 7.0), Align2::LEFT_CENTER, app.tr(word), FontId::proportional(11.0), c.dim);
        }
        y += GROUP_H;
        for pick in picks {
            let row = Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), ENT_H));
            if row.top() < top || row.bottom() > bottom {
                if row.top() < top {
                    above += 1
                } else {
                    below += 1
                }
                y += ENT_H;
                flat += 1;
                continue;
            }
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
            let mut x = row.left() + 14.0 + 18.0;
            if let Pick::State(Some(b)) = pick {
                let (m, col) = bucket_mark(*b);
                x = p.text(pos2(x, cy), Align2::LEFT_CENTER, m, FontId::proportional(12.5), col).right() + 6.0;
            }
            crate::list_ui::filter_words(&p, row, x, pick_label(app, pick), Some(app.acts.count(pick, &app.runs)));
            y += ENT_H;
            flat += 1;
        }
    }
    crate::list_ui::filter_more(&p, app, r, above, below);
}

/// The query and the count over the runs, each folded to fit.
fn runs_col(ui: &Ui, app: &App, r: Rect, out: &mut Out) {
    let c = ito_theme::colors();
    let shown = app.acts.shown(&app.runs);
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(r, 0.0, c.side);
    let head = Rect::from_min_size(r.min, vec2(r.width(), HEAD_H));
    let n = p.text(
        pos2(head.right() - 12.0, head.center().y),
        Align2::RIGHT_CENTER,
        app.tr("list.count").replace("{}", &shown.len().to_string()),
        FontId::proportional(12.0),
        c.dim,
    );
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
    let f = fold(&vec![ROW_FULL; shown.len()], MORE_H, body.height(), sel);
    let now = now_epoch();
    let mut y = body.top();
    if f.above > 0 {
        more(&p, app, body, y, f.above, "list.above");
        y += MORE_H;
    }
    for (k, run) in shown.iter().enumerate() {
        if f.rows[k] == Show::Hidden {
            continue;
        }
        let row = Rect::from_min_size(pos2(body.left(), y), vec2(body.width(), ROW_FULL));
        let resp = ui::hit(ui, row, ("acts-run", k));
        if resp.clicked() {
            out.click = Some(Click::ActRun(k));
        }
        let watched = app.watching(run.id).then(|| app.tr("run.watched"));
        run_row(&p, run, row, k == sel, resp.hovered(), now, watched);
        y += ROW_FULL;
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
        q.push(minato_core::list::users(&app.owners_shown()));
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
/// branch; where, who and when; the duration at the right, and the `w` mark
/// under it while the run is watched.
fn run_row(p: &Painter, run: &Run, row: Rect, on: bool, hover: bool, now: i64, watched: Option<&str>) {
    let c = ito_theme::colors();
    if on {
        p.rect_filled(row, 0.0, style::sel());
        p.rect_filled(Rect::from_min_size(row.min, vec2(3.0, row.height())), 0.0, ACCENT);
    } else if hover {
        p.rect_filled(row, 0.0, c.hover());
    }
    p.line_segment([row.left_bottom(), row.right_bottom()], Stroke::new(1.0, c.border));
    let p = p.with_clip_rect(row);
    let y1 = row.top() + 8.0 + 9.0;
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
    let y2 = y1 + 9.0 + 4.0 + 9.0;
    let wx = style::chip(&p, x, y2, &run.workflow, FontId::monospace(11.0), 6.0, 17.0, 4, c.border, style::head(), c.fg) + 6.0;
    let g = style::truncated(&p, format!("{}  · {}", run.event, run.branch), FontId::monospace(11.0), c.dim, rx - wx);
    p.galley(pos2(wx, y2 - g.size().y / 2.0), g, c.dim);
    let y3 = y2 + 9.0 + 3.0 + 8.0;
    let right = match watched {
        Some(w) => p.text(pos2(right, y3), Align2::RIGHT_CENTER, format!("◉ {w}"), FontId::proportional(11.5), ACCENT).left() - 8.0,
        None => right,
    };
    let g = style::truncated(
        &p,
        format!("{} · #{} · {} · {}", run.repo, run.number, run.actor, ago(&run.started_at, now)),
        FontId::proportional(12.0),
        c.dim,
        right - x,
    );
    p.galley(pos2(x, y3 - g.size().y / 2.0), g, c.dim);
}

/// The run as the mock's detail draws it: where and what state; the title
/// and how it went; the next step and the other keys; the jobs, the last ten
/// results, the artifacts and the release's files.
fn detail_col(ui: &Ui, app: &App, r: Rect, out: &mut Out) {
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
    let top = r.top() + 18.0 - app.acts.scroll * LINE + 10.0;
    let mut y = top;
    // The head: the repository, Run, the workflow and its number, the state.
    let mut hx = style::tag(&p, x, y, &run.repo, &run.repo) + 8.0;
    hx = style::run_chip(&p, hx, y, "▶ Run", ACCENT) + 8.0;
    hx = p.text(pos2(hx, y), Align2::LEFT_CENTER, format!("{} #{}", run.workflow, run.number), FontId::monospace(13.5), c.fg).right() + 8.0;
    let color = tone(run.status);
    let words = format!("{} {}", run.status.mark(), app.tr(run.status.word()));
    // The state on a line of its own when it would run past the right.
    if hx + p.layout_no_wrap(words.clone(), FontId::proportional(11.5), color).size().x + 14.0 > x + w {
        (hx, y) = (x, y + 26.0);
    }
    style::chip(&p, hx, y, &words, FontId::proportional(11.5), 7.0, 18.0, 9, color, color.gamma_multiply(0.14), color);
    y += 10.0 + 10.0;
    let title = p.layout(run.title.clone(), FontId::proportional(21.0), c.fg, w);
    let th = title.size().y;
    p.galley(pos2(x + 0.5, y), title.clone(), c.fg);
    p.galley(pos2(x, y), title, c.fg);
    y += th + 8.0;
    let dur = run.seconds(now).map(actions::duration).unwrap_or_default();
    let branch = if actions::has_pr(run) { format!("{} → main", run.branch) } else { run.branch.clone() };
    // The facts in a row, wrapping before one would run past the right.
    let mut mx = x;
    for m in [run.event.clone(), branch, run.sha.clone(), run.actor.clone(), dur, ago(&run.started_at, now)] {
        if !m.is_empty() {
            let g = p.layout_no_wrap(m, FontId::monospace(13.0), c.dim);
            if mx > x && mx + g.size().x > x + w {
                (mx, y) = (x, y + 20.0);
            }
            let gw = g.size().x;
            p.galley(pos2(mx, y + 8.0 - g.size().y / 2.0), g, c.dim);
            mx += gw + 14.0;
        }
    }
    y += 16.0;
    // What to do next: re-run a failure, stop a run that goes.
    let hist = actions::history(&app.runs, run, 10);
    let bad = hist.iter().filter(|h| h.status == Status::Bad).count();
    let note = match (app.ja(), hist.len()) {
        (true, n) => format!("このワークフローは最近 {n} 回で {bad} 回落ちている"),
        (false, 1) => "The last run of this workflow failed".to_owned(),
        (false, n) => format!("{bad} of the last {n} runs of this workflow failed"),
    };
    // The button runs its built-in key; it shows the keys that do it now.
    let ctx = crate::keymap::Ctx::View(app.view);
    let (rerun, cancel) = (app.char_label(ctx, "R"), app.char_label(ctx, "X"));
    let next = match run.status {
        Status::Bad => Some((
            Next { tone: style::BAD, lead: app.tr("detail.next"), label: app.tr("run.next_bad"), note: &note, key: Some(&rerun) },
            'R',
        )),
        Status::Running | Status::Queued => Some((
            Next { tone: style::RUN, lead: app.tr("detail.next"), label: app.tr("run.next_active"), note: "", key: Some(&cancel) },
            'X',
        )),
        Status::Ok | Status::Idle => None,
    };
    if let Some((n, key)) = next {
        let (under, button) = style::next_box(&p, x, y, w, &n);
        y = under;
        if let Some(k) = style::next_click(ui, button, Some(key), r) {
            out.click = Some(Click::Key(k));
        }
    } else {
        y += 12.0;
    }
    let mut keys = vec![("e", app.tr("key.logs"))];
    if actions::has_pr(run) {
        keys.push(("p", app.tr("run.open_pr")));
    }
    keys.extend([("D", app.tr("key.dispatch")), ("o", app.tr("key.browser"))]);
    y = style::alt_keys(&p, x, y, x + w, &app.alt_labels(&keys));
    let have = app.run_detail.as_ref().filter(|d| d.id == run.id);
    y = style::heading(&p, x, y, app.tr("run.jobs"));
    match have.and_then(|d| d.jobs.as_ref()) {
        None => {
            p.text(pos2(x, y + 9.0), Align2::LEFT_CENTER, app.tr("detail.fetching"), FontId::proportional(13.5), c.dim);
            y += 18.0;
        }
        Some(jobs) => match have
            .and_then(|d| d.workflow.as_ref())
            .filter(|w| w.layers().iter().all(|&l| l < MAX_COLS) && w.jobs.iter().any(|j| !j.needs.is_empty()))
        {
            Some(wf) => y += graph(&p, jobs, wf, pos2(x, y), w, now),
            None => {
                for j in jobs {
                    let d = j.seconds(now).map(actions::duration).unwrap_or_default();
                    y = style::check_row(&p, x, y, w, j.status.mark(), tone(j.status), &j.name, &d) + 2.0;
                }
            }
        },
    }
    // The last results (up to ten, as many as are loaded), newest at the left.
    let head = match (app.ja(), hist.len()) {
        (true, n) => format!("直近 {n} 回"),
        (false, 1) => "Last run".to_owned(),
        (false, n) => format!("Last {n} runs"),
    };
    y = style::heading(&p, x, y, &head);
    for (k, h) in hist.iter().enumerate() {
        let sq = Rect::from_min_size(pos2(x + 10.0 + k as f32 * 18.0, y), vec2(14.0, 14.0));
        p.rect_filled(sq, CornerRadius::same(3), tone(h.status));
    }
    let hx = x + 10.0 + hist.len() as f32 * 18.0 + 8.0;
    if bad > 0 {
        p.text(
            pos2(hx, y + 7.0),
            Align2::LEFT_CENTER,
            if app.ja() { format!("落ちたのは {bad} 回") } else { format!("{bad} failed") },
            FontId::proportional(12.0),
            c.dim,
        );
    } else if !hist.is_empty() {
        p.text(pos2(hx, y + 7.0), Align2::LEFT_CENTER, app.tr("run.all_ok"), FontId::proportional(12.0), c.dim);
    }
    y += 14.0;
    if let Some(arts) = have.and_then(|d| d.artifacts.as_ref()).filter(|a| !a.is_empty()) {
        y = style::heading(&p, x, y, app.tr("run.artifacts"));
        for a in arts {
            artifact(&p, x, y + 10.0, w, &a.name, a.bytes);
            y += 22.0;
        }
    }
    // The files of the release the run made, with its tag.
    if let Some(rel) = have.and_then(|d| d.release.as_ref()) {
        y = style::heading(&p, x, y, &format!("{} · {}", app.tr("run.release"), rel.tag));
        if rel.files.is_empty() {
            p.text(pos2(x + 10.0, y + 10.0), Align2::LEFT_CENTER, app.tr("run.release_none"), FontId::proportional(12.0), c.dim);
            y += 22.0;
        }
        for a in &rel.files {
            artifact(&p, x, y + 10.0, w, &a.name, a.bytes);
            y += 22.0;
        }
    }
    // The keys stop when the end is a little above the foot.
    out.detail_max = Some(((y - top + 36.0 - r.height()) / LINE).ceil().max(0.0));
}

/// One artifact on the line at `cy`: its mark and name at the left, its size
/// at the right. A name too long for the room between is cut short with `…`
/// so it never runs under the size. Returns where the name and the size went.
pub(crate) fn artifact(p: &Painter, x: f32, cy: f32, w: f32, name: &str, bytes: u64) -> (Rect, Rect) {
    let c = ito_theme::colors();
    let font = FontId::monospace(13.0);
    let size = p.text(pos2(x + w - 10.0, cy), Align2::RIGHT_CENTER, actions::size(bytes), font.clone(), c.dim);
    let m = p.text(pos2(x + 10.0, cy), Align2::LEFT_CENTER, "▤", font.clone(), c.fg);
    let at = m.right() + 10.0;
    let g = style::truncated(p, name.to_owned(), font, c.fg, size.left() - 12.0 - at);
    let r = Rect::from_min_size(pos2(at, cy - g.size().y / 2.0), g.size());
    p.galley(r.min, g, c.fg);
    (r, size)
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
    let deepest = rows.iter().copied().max().unwrap_or(0);
    for (k, s) in src.iter().enumerate() {
        let Some(s) = s else { continue };
        for need in &wf.jobs[*s].needs {
            let Some(from) = wf.jobs.iter().position(|j| &j.key == need) else { continue };
            for (m, r) in rect.iter().enumerate() {
                if src[m] == Some(from) {
                    let line = edge(r.right_center(), rect[k].left_center(), at.y, deepest);
                    p.add(egui::Shape::line(line, Stroke::new(1.0, c.dim)));
                }
            }
        }
    }
    for (job, r) in jobs.iter().zip(&rect) {
        let t = tone(job.status);
        p.rect_filled(*r, CornerRadius::same(5), t.gamma_multiply(0.16));
        p.rect_stroke(*r, CornerRadius::same(5), Stroke::new(1.0, t), egui::StrokeKind::Inside);
        job_label(p, *r, &format!("{} {}", job.status.mark(), job.name));
        let clip = p.with_clip_rect(r.shrink(3.0));
        let d = job.seconds(now).map(actions::duration).unwrap_or_default();
        clip.text(pos2(r.left() + 7.0, r.top() + 27.0), Align2::LEFT_CENTER, d, FontId::monospace(10.5), c.dim);
    }
    deepest as f32 * (BOX_H + BOX_GAP)
}

/// A job's mark and name on the top line of its box `r`, cut short with `…`
/// when it is wider than the box. Returns where the text went.
fn job_label(p: &Painter, r: Rect, text: &str) -> Rect {
    let c = ito_theme::colors();
    let g = style::truncated(p, text.to_owned(), FontId::proportional(11.5), c.fg, r.width() - 14.0);
    let at = pos2(r.left() + 7.0, r.top() + 12.0 - g.size().y / 2.0);
    let used = Rect::from_min_size(at, g.size());
    p.galley(at, g, c.fg);
    used
}

/// The line from the right of one box to the left of a box further right.
/// Into the next column it turns in the gap between the two; past a column
/// it runs along the gap under one of the `rows` rows (the one nearest the
/// two ends), so it crosses no box in the columns between.
fn edge(a: egui::Pos2, b: egui::Pos2, top: f32, rows: usize) -> Vec<egui::Pos2> {
    if b.x - a.x <= COL_GAP + 1.0 {
        let mid = a.x + (b.x - a.x) / 2.0;
        return vec![a, pos2(mid, a.y), pos2(mid, b.y), b];
    }
    let half = (a.y + b.y) / 2.0;
    let y = (1..=rows.max(1))
        .map(|n| top + n as f32 * (BOX_H + BOX_GAP) - BOX_GAP / 2.0)
        .min_by(|p, q| (p - half).abs().total_cmp(&(q - half).abs()))
        .unwrap_or(top);
    let (x1, x2) = (a.x + COL_GAP / 2.0, b.x - COL_GAP / 2.0);
    vec![a, pos2(x1, a.y), pos2(x1, y), pos2(x2, y), pos2(x2, b.y), b]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_artifact_name_stops_before_its_size() {
        let long = "windows-arm64-smoke-168aaf65ab120a3dd4a1bf3a9badf8b19c6390f5";
        let ctx = egui::Context::default();
        let (mut got, mut full) = (Vec::new(), 0.0);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let p = ui.ctx().layer_painter(egui::LayerId::background());
            for (name, w) in [(long, 500.0), ("a.zip", 500.0), (long, 2000.0)] {
                got.push(artifact(&p, 0.0, 10.0, w, name, 25_600));
            }
            full = p.layout_no_wrap(long.into(), FontId::monospace(13.0), Color32::WHITE).size().x;
        });
        // No renderer takes the font texture here.
        out.textures_delta.clear();
        let [(cut, size), (short, short_size), (whole, whole_size)] = got[..] else { panic!("three rows") };
        // The long name is cut with room to spare before the size.
        assert!(cut.right() <= size.left() - 10.0, "{cut:?} runs into {size:?}");
        assert!(cut.width() < full - 50.0, "{cut:?} is not cut ({full})");
        // A name that fits, or a row wide enough, keeps every letter.
        assert!(short.width() > 30.0 && short.right() < short_size.left());
        assert_eq!(whole.width(), full);
        assert!(whole.right() < whole_size.left());
    }

    #[test]
    fn a_long_job_name_ends_inside_its_box() {
        let long = "✓ arm64 / windows-arm64-smoke";
        let boxed = |w: f32| Rect::from_min_size(pos2(0.0, 0.0), vec2(w, BOX_H));
        let ctx = egui::Context::default();
        let (mut got, mut full) = (Vec::new(), 0.0);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let p = ui.ctx().layer_painter(egui::LayerId::background());
            for (text, w) in [(long, 120.0), ("✓ sums", 120.0), (long, 600.0)] {
                got.push(job_label(&p, boxed(w), text));
            }
            full = p.layout_no_wrap(long.into(), FontId::proportional(11.5), Color32::WHITE).size().x;
        });
        out.textures_delta.clear();
        let [cut, short, whole] = got[..] else { panic!("three boxes") };
        // The long name is cut and stays off the box's right edge.
        assert!(cut.right() <= boxed(120.0).right() - 7.0, "{cut:?} runs out of its box");
        assert!(cut.width() < full - 30.0, "{cut:?} is not cut ({full})");
        // A short name, or a box wide enough, keeps every letter.
        assert!(short.width() > 20.0 && short.right() < boxed(120.0).right());
        assert_eq!(whole.width(), full);
    }

    #[test]
    fn a_line_past_a_column_keeps_out_of_its_boxes() {
        // Three columns 100 wide: a box in each of the first row, and the
        // middle one has a second row.
        let col = |i: f32| i * (100.0 + COL_GAP);
        let mid_boxes = [0.0, 1.0].map(|n| Rect::from_min_size(pos2(col(1.0), n * (BOX_H + BOX_GAP)), vec2(100.0, BOX_H)));
        let line = edge(pos2(col(0.0) + 100.0, BOX_H / 2.0), pos2(col(2.0), BOX_H / 2.0), 0.0, 2);
        for seg in line.windows(2) {
            for r in &mid_boxes {
                // Each piece is level or upright, so its box of bounds is the piece.
                let s = Rect::from_two_pos(seg[0], seg[1]);
                assert!(!s.intersects(r.shrink(0.5)), "{seg:?} crosses {r:?}");
            }
        }
        assert_eq!(line.first(), Some(&pos2(100.0, BOX_H / 2.0)));
        assert_eq!(line.last(), Some(&pos2(col(2.0), BOX_H / 2.0)));
        // The next column: one turn in the gap, as before.
        assert_eq!(edge(pos2(100.0, 10.0), pos2(col(1.0), 50.0), 0.0, 2).len(), 4);
    }
}
