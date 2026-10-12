//! Painting of the dispatch form and the job log (see `overlay.rs`).

use egui::{Align2, CornerRadius, FontId, Rect, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::app::App;
use crate::overlay::{Form, LogView, Overlay};
use crate::ui::ACCENT;

const ROW: f32 = 20.0;
const FIELD_H: f32 = 46.0;

pub fn show(ui: &mut Ui, app: &App) {
    let Some(o) = &app.overlay else { return };
    let c = ito_theme::colors();
    let r = ui.max_rect();
    let p = ui.painter();
    p.rect_filled(r, 0.0, egui::Color32::from_black_alpha(110));
    let b = r.shrink2(vec2((r.width() * 0.12).max(20.0), (r.height() * 0.1).max(20.0)));
    p.rect_filled(b, CornerRadius::same(8), c.panel);
    p.rect_stroke(b, CornerRadius::same(8), Stroke::new(1.0, ACCENT), StrokeKind::Inside);
    let inner = b.shrink(16.0);
    let p = p.with_clip_rect(inner);
    match o {
        Overlay::Form(f) => form(&p, app, f, inner),
        Overlay::Log(l) => log(&p, app, l, inner),
    }
}

fn form(p: &egui::Painter, app: &App, f: &Form, r: Rect) {
    let c = ito_theme::colors();
    p.text(r.left_top() + vec2(0.0, 10.0), Align2::LEFT_CENTER, format!("{}  {} · {}", app.tr("dispatch.title"), f.run.workflow, f.run.branch), FontId::proportional(14.0), c.fg);
    p.text(pos2(r.left(), r.bottom() - 8.0), Align2::LEFT_CENTER, app.tr("dispatch.hint"), FontId::proportional(11.0), c.dim);
    let mut y = r.top() + 30.0;
    let fit = (((r.height() - 56.0) / FIELD_H) as usize).max(1);
    let first = f.sel.saturating_sub(fit - 1);
    for (k, i) in f.inputs.iter().enumerate().skip(first).take(fit) {
        let on = k == f.sel;
        let row = Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), FIELD_H - 4.0));
        if on {
            p.rect_filled(row, CornerRadius::same(4), ACCENT.gamma_multiply(0.14));
        }
        let req = if i.required { " *" } else { "" };
        p.text(row.left_top() + vec2(8.0, 12.0), Align2::LEFT_CENTER, format!("{}{req}", i.name), FontId::proportional(12.5), if on { ACCENT } else { c.fg });
        p.text(row.right_top() + vec2(-8.0, 12.0), Align2::RIGHT_CENTER, &i.description, FontId::proportional(11.0), c.dim);
        let v = &f.values[k];
        let shown = if Form::picks(i) {
            if i.kind == "boolean" { format!("{} {}", if v == "true" { "[x]" } else { "[ ]" }, v) } else { format!("< {v} >") }
        } else if on {
            format!("{v}▏")
        } else {
            v.clone()
        };
        p.text(row.left_top() + vec2(8.0, 32.0), Align2::LEFT_CENTER, shown, FontId::monospace(12.0), c.fg);
        y += FIELD_H;
    }
}

fn log(p: &egui::Painter, app: &App, l: &LogView, r: Rect) {
    let c = ito_theme::colors();
    p.text(r.left_top() + vec2(0.0, 10.0), Align2::LEFT_CENTER, format!("{}  {}", app.tr("log.title"), l.name), FontId::proportional(14.0), c.fg);
    p.text(pos2(r.left(), r.bottom() - 8.0), Align2::LEFT_CENTER, app.tr("log.hint"), FontId::proportional(11.0), c.dim);
    let body = Rect::from_min_max(pos2(r.left(), r.top() + 28.0), pos2(r.right(), r.bottom() - 24.0));
    let Some(lines) = &l.lines else {
        p.text(body.left_top() + vec2(0.0, 10.0), Align2::LEFT_CENTER, app.tr("detail.fetching"), FontId::proportional(12.5), c.dim);
        return;
    };
    let p = p.with_clip_rect(body);
    let fit = ((body.height() / ROW) as usize).max(1);
    for (k, line) in lines.iter().enumerate().skip(l.scroll).take(fit) {
        let color = if line.starts_with("##[error]") { crate::style::BAD } else if line.starts_with("##[") { c.dim } else { c.fg };
        p.text(pos2(body.left(), body.top() + (k - l.scroll) as f32 * ROW + ROW / 2.0), Align2::LEFT_CENTER, line, FontId::monospace(11.5), color);
    }
    let pos = if app.ja() { format!("{} / {} 行", l.scroll + 1, lines.len()) } else { format!("{} / {}", l.scroll + 1, lines.len()) };
    p.text(pos2(r.right(), r.bottom() - 8.0), Align2::RIGHT_CENTER, pos, FontId::monospace(11.0), c.dim);
}
